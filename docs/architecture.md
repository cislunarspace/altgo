# 系统架构

本文档是 altgo 的架构速览，面向维护者与贡献者，与 [`testing.md`](testing.md) 配套阅读。数据来自 2026-08-10 对 `src-tauri/src/` 的源码调研。

## 一、系统概览

altgo 是基于 Tauri 的桌面语音转文字工具：Rust 后端承载整条语音流水线，React 前端负责设置页、历史页与悬浮窗。

核心设计只有一句话：**业务核心与框架彻底解耦，平台差异用 trait + `cfg` 隔离**。`voice_pipeline` 模块完全不 import Tauri，只通过 `PipelineSink`、`TranscriptionDispatch`、`OverlaySink` 等 trait seam 与外界交互；平台相关的按键监听、录音、剪贴板也都被收进各自 trait 后面。

整条流水线跑在独立的 OS 线程上，内部是一个独立的 `current_thread` tokio 运行时，与 Tauri 主运行时隔离（`lib.rs:56-60`）。

```text
+-----------------+
|   React 前端     |  设置 / 历史 / 浮窗
+-----------------+
         | IPC (16 命令 + 9 事件)
         v
+-----------------+
|  装配层 lib.rs   |  spawn_pipeline_thread：管理状态、组合 seam
+-----------------+
         |
         v
+-----------------------------+
|     voice_pipeline          |  组合根：builder + context + handlers
|  ┌───────────────────────┐  |
|  |   PipelineContext     |  |  tokio::select! 三分支主循环
|  |  ┌─────────────────┐  |  |
|  |  |   state_machine  |  |  |  5 态同步状态机
|  |  └─────────────────┘  |  |
|  |  handlers.rs          |  |  录音→转写→润色→分发
|  |  dispatcher.rs        |  |  TranscriptionDispatch seam
|  |  sink.rs              |  |  PipelineSink / TranscriptionResult
|  └───────────────────────┘  |
+-----------------------------+
         |                  |
         v                  v
+-------------------+  +---------------------------+
| 转写/润色后端      |  |      平台适配层            |
| transcriber       |  | key_listener              |
| whisper_server    |  | recorder                  |
| polisher          |  | key_capture               |
| prompt_store      |  | output                    |
| model             |  +---------------------------+
+-------------------+
         |
         v
+----------------------+
| config/error/resource |  公共叶子
+----------------------+
```

## 二、依赖方向

依赖整体单向、清晰：

- 装配层 `lib.rs` 唯一负责把 Tauri-managed state 注入流水线。
- `voice_pipeline` 是组合根（composition root），内部再向下组合 `state_machine` / `handlers` / `dispatcher` / `sink`。
- `handlers` 调用 `transcriber` / `polisher` / `recorder`。
- `transcriber` 调用 `whisper_server` 与 `resource`。
- `polisher` 调用 `prompt_store`。
- `model` / `config` / `error` / `resource` 是底层叶子。

```text
lib.rs
  |
  +-- voice_pipeline
        |
        +-- state_machine  (纯同步叶子)
        +-- handlers
        |     +-- transcriber
        |     |     +-- whisper_server
        |     |     +-- resource
        |     +-- polisher
        |     |     +-- prompt_store
        |     +-- recorder
        +-- dispatcher
        +-- sink

  +-- pipeline_controller  (生命周期 + PipelineStatus)
  +-- tauri_sink           (持有 AppHandle)
  +-- cmd.rs               (IPC 命令)
```

三个值得注意的依赖关系：

1. **唯一一个环**：`polisher` ↔ `prompt_store`，但二者同处一个 crate 内，可接受。
2. `voice_pipeline::context.rs` 依赖 `pipeline_controller::PipelineStatus`（UI 状态枚举从底层向上泄漏了一点）。
3. `tauri_sink.rs` 直接攥着 `tauri::AppHandle`（issue #104 要修的已知问题）。

## 三、核心流水线

### 主循环

`voice_pipeline/context.rs:71-90` 是 tokio::select! 三分支：

1. **按键事件** → `machine.process(ev)`。
2. **状态机超时** → `machine.poll_timeout()`（仅在 `deadline.is_some()` 时启用）。
3. **停止信号** → `break`。

命令由状态机同步返回，`match cmd` 后**就地**调用 `handle_start_record` 或 `handle_stop_record`。**没有独立的“命令通道”**——状态机不自己执行副作用，只是把意图交给调用方。

### 状态机

`state_machine.rs` 是纯同步叶子，5 个状态：

| 状态 | 含义 |
|------|------|
| `Idle` | 空闲 |
| `PotentialPress` | 按下后等待是否达到长按阈值 |
| `Recording` | 长按触发，松开即停 |
| `WaitSecondClick` | 短按松开后等待双击 |
| `ContinuousRecording` | 双击触发，再按一次停止 |

状态机通过 `next_deadline()` 把下一个超时点暴露给外层（`state_machine.rs:191-198`），由 `tokio::time::sleep_until` 驱动。

### 录音 → 转写 → 润色 → 分发

`handlers.rs:34-117` 的调用链如下：

| 步骤 | 输入 | 输出/行为 | 出错处理 |
|------|------|-----------|----------|
| 停止录音 | `&dyn Recorder` | WAV 字节 | 报错，回 `Idle` |
| 转写 | WAV + 进度回调 | `TranscribeResult` | `on_error` + 回 `Idle` |
| 空文本过滤 | 转写文本 | 直接 `done` | 仅跳过 |
| 润色 | `raw_text` + `LLMFormatter` | 润色后文本 | 降级为 `raw_text` |
| 结果分发 | `TranscriptionResult` | 浮窗 + 剪贴板 + 历史 | 剪贴板/历史失败只 warn |

关键点：

- 转写失败、润色失败都是**可恢复降级**。
- 剪贴板失败、历史追加失败只 `tracing::warn!`，不中断结果返回（`handlers.rs:184-199`）。

### 转写回退是一张网

`engine = "local"` 时走 `ResidentWhisper`：管道启动时一次性拉起 `whisper-server`，模型常驻内存，之后每句话只发一次本地 HTTP 请求。`whisper_server.rs:165-192` 保证任何失败都回退到一次性 `LocalWhisper`：

- 二进制缺失
- 端口冲突
- 就绪超时（`READY_TIMEOUT = 120s`）
- 运行期崩溃
- 本地 HTTP 推理失败

`ResidentWhisper::new()` 永不返回错误；`transcribe()` 永远有兜底。**没有独立的“常驻 vs 一次性”开关**——这是 local 引擎内建的双层策略。

### 润色 prompt 三级回退

`polisher.rs` 构造 `LLMFormatter` 时，`from_config_with_sources`（`polisher.rs:327`）统一驱动三级回退（`build_prompt_source_chain`，`polisher.rs:528-558`）：

1. `PromptStore` 加载的 `resources/prompts/` 模板（`base.txt` + 档级后缀）。
2. 配置里的 `system_prompt`（非空时）。
3. 内置 hardcoded 默认提示。

启动时加载一次，改文件需重启生效。

### 主循环阻塞是有意设计

`handle_stop_record` 在 select 分支内被 `await`，一次转写/润色会阻塞整个按键循环直到完成。这是**有意设计**——altgo 的转写是“按一次键录一句”的单发操作，阻塞保证一次只完成一次转写。详见 ADR-0003。

## 四、错误模型

`error.rs` 把错误分为两层：

| 分类 | 用途 | 典型枚举 |
|------|------|----------|
| `FatalError` | 构建期，管道不启动 | `ModelNotFound` / `ApiAuthFailed` / `KeyListenerFailed` / `TranscriberInitFailed` / `PolisherInitFailed` / `RecorderInitFailed` |
| `RecoverableError` | 运行时，降级继续 | `TranscriptionFailed` / `PolishingFailed` / `RecordingFailed` / `EmptyTranscription` |

trait 边界一律返回自定义 thiserror 枚举：`RecorderError` / `OutputError` / `KeyListenerError` / `ModelError` / `ConfigError` / `HistoryError`。`recorder/mod.rs:45-52` 专门测试防止回退到 `anyhow`。

一个小不一致：运行时 handler 里的错误实际走 `to_string()` + `sink.on_error`，结构化 `PipelineError` 主要用于构建期——两套机制并行存在。

## 五、平台抽象

平台差异通过 **trait + `cfg` 类型别名** 隔离：

| 模块 | Trait | Linux 实现 | Windows 实现 |
|------|-------|------------|--------------|
| `key_listener` | `KeyListener::start()` | `xinput test-xi2` / `evtest` | `WH_KEYBOARD_LL` 独立消息泵线程 |
| `recorder` | `Recorder::start/stop/is_recording` | `parecord` 子进程 | `cpal` WASAPI + `rubato` 重采样 |
| `output` | `Output::write_clipboard` + `clone_box` | `xclip`/`xsel`/`wl-copy` 探测一次 | `arboard`（`!Send`，走 `spawn_blocking`） |
| `key_capture` | 无（`cfg` 自由函数） | `evtest` 枚举 `/dev/input` 等一次按键 | 一次性 `WH_KEYBOARD_LL` 钩子消息泵 |

`recorder/dsp.rs` 是平台无关层，重采样、降混、WAV 封装可在 Linux 上编译测试。

`Box<dyn Trait>` 用于 `PipelineContext` 字段与 builder 返回值；`Arc<dyn Trait>` 用于 Tauri 侧注入。

## 六、IPC 契约面

### 命令

`cmd.rs` 暴露 16 个命令：

- 配置 3：`get_config`、`save_config`、`capture_activation_key`
- 流水线 3：`start_pipeline`、`stop_pipeline`、`get_status`
- 浮窗 2：`copy_text`、`hide_overlay`
- 模型 4：`list_models`、`download_model`、`delete_model`、`resolve_model`
- 历史 4：`list_history`、`delete_history_entries`、`clear_history`、`polish_history_entry`

注意：`get_status` 与 `stop_pipeline` 已注册但前端零调用，疑为死代码。

### 事件

共 9 个 Tauri 事件：

`pipeline-status`、`pipeline-error`、`transcription-result`、`transcription-progress`、`key-listener-backend`、`history-updated`、`overlay-state`、`model-download-progress`、`model-download-finished`。

### 序列化契约

- IPC 与 `history.json` 使用 `camelCase`。
- `config.toml` 使用 `snake_case`。
- 前端永远只见 camelCase；Rust 内部 snake_case，靠 `serde(rename_all)` 在边界转换。

## 七、质量 / 可维护性观察点

### 结构性

- `tauri_sink` 直接持有 `AppHandle`（`tauri_sink.rs:36`；#104）。
- `config_store::apply_patch` 不是原子更新：校验失败时内存已部分应用、不落盘。
- `key_capture` 无 trait，平台实现直接暴露自由函数，测试隔离性稍弱。

### 文档漂移 / 死代码

- `notify-send` 从未实现，结果展示统一走 Tauri overlay。
- `prompt_store` 没有热重载，改文件需重启。
- `get_status` / `stop_pipeline` 疑为死代码。
- `testing.md` 列出的测试缺口：模型下载、MimoAsr、Anthropic 协议、`whisper_server` 崩溃回退、`cmd.rs` 零测试、`handle_stop_record` 成功链路、`PipelineContext` 端到端。

### 架构资产

- `voice_pipeline` 完全不 import Tauri，全靠 trait seam。
- 状态机、`overlay/manager`、`recorder/dsp` 都是干净的叶子/纯逻辑，测试覆盖好。
- 回退链设计成熟：转写网、`prompt` 三级、模型下载官方→镜像。
- 错误分类致命/可恢复边界清晰。
