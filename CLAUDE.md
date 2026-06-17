## 交流语言

始终使用中文与用户交流。代码、commit message、PR 描述等技术输出也用中文。

## 写作要求

所有面向人读的文本——CONTEXT.md、ADR、issue 评论、PR 描述、agent brief、triage notes、Sphinx 文档——遵守以下原则：

- **善于总结材料**：材料弄全弄准，去粗取精、去伪存真、由此及彼、由表及里，反映事物本质；不堆砌细节、不拼凑清单。
- **不用夸大的修饰词**：不写"权威""强大""完整""单一事实来源"之类的修饰，它们减损力量。
- **注意词语的逻辑界限**：相邻概念要划清，不混用、不模糊。
- **废话应当尽量除去**。
- **通俗、亲切，由小讲到大，由近讲到远，引人入胜**：先讲读者已知／当前的事物，再推到陌生／抽象的；忌一上来就宏大叙事或先搬死人、外国人。
- **与读者完全平等**：靠分析说服，不要装腔作势来吓人；老老实实办事。

## 项目概览

**altgo** 是用 Rust 编写的桌面语音转文字工具，支持 **Linux**（Ubuntu 20.04+）和 **Windows**（通过 GitHub Releases 提供 MSI）。按住右 Alt 键录音，松开后使用 **本地 whisper.cpp** 进行转写，可选择通过任意 **兼容 OpenAI 的 LLM** API 进行润色，然后将结果写入系统剪贴板，并在悬浮浮窗中显示（如果剪贴板工具失败，则使用浮窗副本作为回退）。成功的转写结果（原始文本 + 显示文本）以纯文本历史记录的形式持久化保存在本地 JSON 文件（`~/.config/altgo/history.json`）中；音频不会被保存。代码中可能仍包含可选的 HTTP Whisper API 路径，以供高级用途。

## 构建与测试命令

```bash
# 仅 Rust（无 GUI）
cargo build --release --manifest-path=src-tauri/Cargo.toml
cargo test --manifest-path=src-tauri/Cargo.toml
cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings

# Tauri GUI 模式
cargo tauri dev               # 开发模式（前端开发服务器 + 桌面窗口）
cargo tauri build            # 生产环境 GUI 构建

# make build：先运行 ensure-binary-deps（deps-linux），
# 然后执行 cargo tauri build，再把 target/deps/bin/* 复制到 src-tauri/target/release/bin/
make build
make install                  # 构建后：altgo -> /usr/local/bin，deps -> /usr/lib/altgo/bin，config -> /etc/altgo/
```

### Windows 构建

```powershell
# Windows 上等价于 `make build`（下载依赖 + cargo tauri build --bundles msi）
.\build.ps1
# 或：pwsh packaging/scripts/build.ps1
# 或：build.cmd（未将 pwsh 加入 PATH 的系统使用的包装脚本）

# 测试/lint 命令与 Linux 相同
cargo test --manifest-path=src-tauri/Cargo.toml
cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings
```

## 架构

基于 Tauri 的桌面应用，核心逻辑位于 `src-tauri/src/`：

| 组件 | 路径 |
|-----------|------|
| **Tauri GUI** | `src-tauri/` + `frontend/` |
| **核心模块** | `src-tauri/src/` |

由键盘事件驱动的核心流水线：

```
Key Listener → State Machine → Recorder → Transcriber → Polisher → Output (+ History JSON)
```

### 模块（位于 `src-tauri/src/`）

- **`lib.rs`** —— Tauri 应用入口：`run()` 运行循环、`AppState`（`config_path`、**`history_path`**、pipeline 句柄与状态），以及 `spawn_pipeline_thread`（在独立 OS 线程的 tokio 运行时上拉起 `voice_pipeline::run`）。
- **`cmd.rs`** —— 通过 IPC 暴露给前端的 Tauri 命令：配置（`get_config`、`save_config`、`capture_activation_key`）、pipeline（`start_pipeline`、`stop_pipeline`、`get_status`）、浮窗（`copy_text`、`hide_overlay`）、模型（`list_models`、`download_model`、`delete_model`、`resolve_model`）、**历史记录**（`list_history`、`delete_history_entries`、`clear_history`、`polish_history_entry`）。语音流水线（`run_pipeline`）在 `raw_text` 非空时追加一行，写入成功后发出 `history-updated` 事件，并优先在润色后文本经 trim 后非空时展示润色文本。
- **`history.rs`** —— `HistoryStore`：对 `history.json` 的追加/列出/删除/清空/更新/计数（camelCase JSON，文件 I/O 使用 `Mutex`）。`HistoryStore` 是唯一对外接口，调用方不直接接触文件路径或内部辅助函数。不保存音频。
- **`config.rs`** —— 使用 `serde(default)` 加载每个字段的 TOML 配置；`ConfigPatch` 补丁逻辑与字段定义共处一处。API 密钥可通过环境变量覆盖（例如 `ALTGO_POLISHER_API_KEY`；若使用 API 引擎，则转写器密钥同样可覆盖）。
- **`config_store.rs`** —— `Config` 的持久化封装；所有变更经 `apply_patch` 原子校验并写盘。
- **`state_machine.rs`** —— 5 状态枚举（`Idle`、`PotentialPress`、`Recording`、`WaitSecondClick`、`ContinuousRecording`）。长按录音，双击进入连续模式。使用 `tokio::select!` 让按键事件与超时竞争。
- **`audio.rs`** —— 线程安全的 PCM 缓冲区（`Mutex<Vec<u8>>`），WAV 编码/解码（44 字节头 + PCM）。
- **`error.rs`** —— 类型化管道错误（`PipelineError`），区分致命（停管道）与可恢复（降级），中英双语消息。
- **`transcriber.rs`** —— 转写后端 trait 与实现：`WhisperApi`（HTTP multipart 上传至兼容 OpenAI 的端点）、`LocalWhisper`（一次性 `whisper-cli` 子进程）。本地默认走常驻后端（见 `whisper_server.rs`）。
- **`whisper_server.rs`** —— 常驻 whisper-server 管理（`ResidentWhisper`）：管道启动时拉起一次，模型常驻内存，逐句走本地 HTTP；二进制缺失、端口冲突、就绪超时或运行期崩溃均自动回退到一次性 `LocalWhisper`。
- **`polisher.rs`** —— 使用 LLM 对文本进行 4 档润色（`none`/`light`/`medium`/`heavy`），支持 OpenAI 兼容聊天 API 与 Anthropic Messages API 两种协议。指数退避重试（3 次）。
- **`prompt_store.rs`** —— 润色 prompt 模板管理：从 `resources/prompts/` 组合 `base.txt` + 各档后缀，文件变动时热重载（去抖 500ms）。
- **`voice_pipeline.rs`** —— 核心处理流水线（录音→转写→润色）的单一深模块。定义 `PipelineSink` 接缝（状态变更、错误、结果、进度、按键后端通知）与 `PipelineOutput`；剪贴板注入与历史写入集中在 `process_transcription_result`。
- **`pipeline_controller.rs`** —— 流水线生命周期与状态跟踪（`PipelineStatus`：Idle/Recording/Processing 等），对应 `start`/`stop`/`get_status`。
- **`tauri_sink.rs`** —— `PipelineSink` 的 Tauri 适配器：把管道事件转成前端事件与剪贴板写入，并把悬浮窗操作委托给 `OverlayManager`。
- **`model.rs`** —— whisper.cpp GGML 模型管理（下载、切换、存储在 `~/.config/altgo/models/`）。
- **`tray.rs`** —— 系统托盘配置（显示窗口、退出菜单）。
- **`resource.rs`** —— 资源文件管理。
- **`key_capture.rs`** —— 设置中的一次性激活键捕获（Linux evdev；Windows WH_KEYBOARD_LL）。
- **`key_listener/`** —— 按键检测（Linux：`xinput test-xi2` / Windows：通过 `SetWindowsHookExW` 在独立消息泵线程上挂接 `WH_KEYBOARD_LL`）。
- **`recorder/`** —— 音频捕获（Linux：`parecord` PulseAudio / Windows：`cpal` WASAPI；输出 16kHz 单声道 WAV；如果设备采样率不同，则通过 rubato 重采样）。
- **`output/`** —— 剪贴板 + 通知（Linux：`xclip`/`xsel`/`wl-copy` + `notify-send` / Windows：`arboard` 剪贴板 + 空操作通知；Windows 由浮窗负责显示）。
- **悬浮窗（`overlay_window.rs` / `overlay_manager.rs` / `tauri_overlay_window.rs`）** —— 状态意图与窗口操作分离：`overlay_window.rs` 定义 `OverlayWindow` seam，`overlay_manager.rs` 按状态意图算尺寸/位置，`tauri_overlay_window.rs` 是 Tauri 适配器（用 `GetMonitorInfoW` 取显示器几何）。

### 前端结构（`frontend/src/`）

```
├── App.tsx                 # 应用入口
├── main.tsx                # React 渲染入口
├── ThemeContext.tsx        # 主题 Provider
├── theme.ts                # 主题 token / 持久化
├── overlay.tsx             # 悬浮窗口组件
├── overlay.css             # 浮窗样式（引入 overlay-base、motion）
├── components/
│   ├── ui/                 # 基础 UI 组件（Input、Button、Card）
│   ├── Layout.tsx          # 布局组件
│   └── StatusIndicator.tsx # 状态指示器
├── pages/
│   ├── Home.tsx            # 首页
│   ├── History.tsx         # 转写历史（选择 / 删除 / 清空 / 复制 / 润色单行）
│   └── Settings.tsx        # 设置页
├── hooks/
│   └── useTauri.ts         # Tauri 集成 hook
├── i18n/                   # 国际化
└── styles/
    ├── global.css
    ├── components.css
    ├── design-system.css
    ├── design-tokens.css   # 设计 token
    ├── motion.css          # 动效 / 过渡
    └── overlay-base.css    # 共享浮窗布局
```

### 关键模式

**基于子进程的系统交互（Linux）** —— Linux 上的平台集成通过调用 CLI 工具（`xinput`、`parecord`、`xclip`）完成。这简化了构建，避免了原生依赖的复杂性。

**Win32 API 绑定（Windows）** —— Windows 使用 `windows` crate（0.61）实现键盘钩子（`WH_KEYBOARD_LL`）和显示器几何信息（`GetMonitorInfoW`），使用 `cpal`（0.17）进行 WASAPI 音频捕获，使用 `arboard`（3）操作剪贴板。这些在 `Cargo.toml` 中都是仅限 `cfg(windows)` 的依赖。

**通过 `cfg` + trait 实现平台抽象** —— 每个平台模块（`key_listener/`、`recorder/`、`output/`）使用 `#[cfg(target_os)]` 选择具体实现。`Platform*` 类型别名提供默认实现，每个模块暴露一个 trait（`KeyListener`、`Recorder`、`Output`），以便流水线可以使用 `Box<dyn Trait>`，提升可测试性。

**异步通道流水线** —— `tokio::sync::mpsc` 通道解耦各阶段。按键事件通过无界通道流动，命令通过有界通道（容量 16）。处理任务作为独立的 `tokio::spawn` 任务启动。

**配置** —— 位于 `~/.config/altgo/altgo.toml`。模板在 `configs/altgo.toml`。所有字段都有 serde 默认值，因此部分配置也能工作。

**转写历史** —— `~/.config/altgo/history.json`（与配置同目录）。条目：`id`、`createdAtMs`、`rawText`、`text`。浮窗和前端监听 **`history-updated`** 事件以刷新列表。

### 系统要求

**Linux**：`xinput`、`xmodmap`、`parecord`、`xclip`/`xsel`/`wl-copy`、`notify-send`

**Windows**：WebView2 Runtime（若缺失，MSI 会自动安装）、麦克风（WASAPI 默认设备）。不依赖 CLI 工具 —— 所有平台集成均使用 Win32 API 或内置 crate。

### 平台特定依赖（Cargo.toml）

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.61", features = [
    "Win32_UI_Input_KeyboardAndMouse", "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation", "Win32_System_Threading", "Win32_Graphics_Gdi",
] }
cpal = "0.17"
arboard = { version = "3", default-features = false }
```

注意：`cpal` 0.17 被固定（而非 0.18），以避免与 tao/tauri 2.10 的 `windows-core` 版本冲突。参见 memory 文件 `windows-cargo-version-pin`。

### Tauri GUI 开发

首次运行前，安装前端依赖：
```bash
cd frontend && npm install
```

## 测试说明

- 单元测试位于每个源文件的 `#[cfg(test)]` 模块内。
- `config.rs`、`audio.rs`、`model.rs` 和 `history.rs` 有全面的测试。
- `transcriber.rs` 和 `polisher.rs` 使用 `mockito` 进行 HTTP 级别的模拟。
- 平台特定模块只有少量测试（仅构造/冒烟测试）。
- **Windows 代码不在 CI 中测试**（仅 Linux CI 运行器）。Windows 特定代码路径在 Windows 机器上手动验证。发布工作流会构建 MSI，但不会在 Windows 上运行 `cargo test`。

## Agent 技能

### Issue tracker

Issue 位于 GitHub Issues（`cislunarspace/altgo`）。使用 `gh` CLI。参见 `docs/agents/issue-tracker.md`。

### Triage labels

五个标准 triage 标签，使用默认名称。参见 `docs/agents/triage-labels.md`。

### Domain docs

单上下文布局（仓库根目录的 `CONTEXT.md` + `docs/adr/`）。参见 `docs/agents/domain.md`。
