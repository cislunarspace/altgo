## 交流语言

始终使用中文与用户交流。代码、commit message、PR 描述等技术输出也用中文。

## 写作要求

所有面向人读的文本——注释、CONTEXT.md、ADR、issue 评论、PR 描述、agent brief、triage notes、Sphinx 文档、Agent 回复——遵守以下原则：

- **善于总结材料**：材料弄全弄准，去粗取精、去伪存真、由此及彼、由表及里，反映事物本质；不堆砌细节、不拼凑清单。
- **不用夸大的修饰词**：不写"权威""强大""完整""单一事实来源"之类的修饰，它们减损力量。
- **注意词语的逻辑界限**：相邻概念要划清（如"配置"与"运行规格"、"力模型"与"力模型聚合"），不混用、不模糊。
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
| **文档站点** | `docs-site/`（Docusaurus） |

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
- **`state_machine.rs`** —— 5 状态枚举（`Idle`、`PotentialPress`、`Recording`、`WaitSecondClick`、`ContinuousRecording`）。长按录音，双击进入连续模式。提供同步接口（`process`、`poll_timeout`、`next_deadline`），由 `voice_pipeline` 的 `tokio::select!` 主循环驱动。
- **`audio.rs`** —— 线程安全的 PCM 缓冲区（`Mutex<Vec<u8>>`），WAV 编码/解码（44 字节头 + PCM）。
- **`error.rs`** —— 类型化错误枚举（`PipelineError`、`OutputError`、`KeyListenerError`、`ModelError`、`ConfigError`、`HistoryError`），区分致命（停管道）与可恢复（降级）。
- **`transcriber.rs`** —— 转写后端 trait 与实现：`WhisperApi`（HTTP multipart 上传至兼容 OpenAI 的端点）、`LocalWhisper`（一次性 `whisper-cli` 子进程）。本地默认走常驻后端（见 `whisper_server.rs`）。
- **`whisper_server.rs`** —— 常驻 whisper-server 管理（`ResidentWhisper`）：管道启动时拉起一次，模型常驻内存，逐句走本地 HTTP；二进制缺失、端口冲突、就绪超时或运行期崩溃均自动回退到一次性 `LocalWhisper`。
- **`polisher.rs`** —— 使用 LLM 对文本进行 4 档润色（`none`/`light`/`medium`/`heavy`），支持 OpenAI 兼容聊天 API 与 Anthropic Messages API 两种协议。指数退避重试（3 次）。`polisher/protocol.rs` 定义 API 协议类型（`ApiProtocol`）。
- **`prompt_store.rs`** —— 润色 prompt 模板管理：从 `resources/prompts/` 组合 `base.txt` + 各档后缀，文件变动时热重载（去抖 500ms）。
- **`voice_pipeline/`** —— 核心处理流水线（录音→转写→润色）的单一深模块。`sink.rs` 定义 `PipelineSink` 接缝（状态变更、错误、结果、进度、按键后端通知）与 `TranscriptionResult` / `DispatchOutcome`；`dispatcher.rs` 是 sink 注入的业务 seam（剪贴板写入 + 历史追加，归到 `TranscriptionDispatch` trait），生产实现 `TranscriptionDispatcherImpl` 转调 `process_transcription_result`；`handlers.rs` 留有 `dispatch_history_polish` 编排 `store.get + formatter.polish + store.polish_entry`。
- **`pipeline_controller.rs`** —— 流水线生命周期与状态跟踪（`PipelineStatus`：Idle/Recording/Processing 等），对应 `start`/`stop`/`get_status`。
- **`tauri_sink.rs`** —— `PipelineSink` 的 Tauri 适配器：把管道事件转成前端事件，并把悬浮窗操作委托给 `OverlayManager`。剪贴板/历史业务由 `TranscriptionDispatch` trait 注入（构造时一次性决定），本模块不再持有 `Output` 或 `HistoryStore`。
- **`model.rs`** —— whisper.cpp GGML 模型管理（下载、切换、存储在 `~/.config/altgo/models/`）。
- **`tray.rs`** —— 系统托盘配置（显示窗口、退出菜单）。
- **`resource.rs`** —— 资源文件管理。
- **`key_capture/`** —— 设置中的一次性激活键捕获（Linux evdev；Windows WH_KEYBOARD_LL）。`mod.rs` 包含共享类型与 Linux 实现，`windows.rs` 为 Windows 实现。
- **`key_listener/`** —— 按键检测（Linux：`xinput test-xi2` / Windows：通过 `SetWindowsHookExW` 在独立消息泵线程上挂接 `WH_KEYBOARD_LL`）。
- **`recorder/`** —— 音频捕获（Linux：`parecord` PulseAudio / Windows：`cpal` WASAPI；输出 16kHz 单声道 WAV；如果设备采样率不同，则通过 rubato 重采样）。
- **`output/`** —— 剪贴板 + 通知（Linux：`xclip`/`xsel`/`wl-copy` + `notify-send` / Windows：`arboard` 剪贴板 + 空操作通知；Windows 由浮窗负责显示）。
- **悬浮窗（`overlay/`）** —— 状态意图与窗口操作分离：`overlay/seam.rs` 定义 `OverlayWindow` seam，`overlay/manager.rs` 按状态意图算尺寸/位置，`overlay/tauri.rs` 是 Tauri 适配器（用 `GetMonitorInfoW` 取显示器几何）。

### 前端结构（`frontend/src/`）

```
├── App.tsx                 # 应用入口
├── main.tsx                # React 渲染入口
├── ThemeContext.tsx        # 主题 Provider
├── theme.ts                # 主题 token / 持久化
├── overlay.tsx             # 悬浮窗口组件
├── overlay.css             # 浮窗样式（由 overlay.tsx 在 TS 侧 import overlay-base、motion）
├── components/
│   ├── Layout.tsx          # 布局组件
│   └── StatusIndicator.tsx # 状态指示器
├── pages/
│   ├── Home.tsx            # 首页
│   ├── History.tsx         # 转写历史（选择 / 删除 / 清空 / 复制 / 润色单行）
│   └── Settings.tsx        # 设置页
├── hooks/
│   ├── useTauri.ts         # Tauri 集成 hook
│   ├── useConfigForm.ts    # 配置表单 hook
│   └── useModelManager.ts  # 模型管理 hook
├── i18n/                   # 国际化
└── styles/
    ├── global.css
    ├── design-system.css
    ├── design-tokens.css   # 设计 token
    ├── motion.css          # 动效 / 过渡
    ├── layout.css          # 布局组件样式
    ├── overlay-base.css    # 共享浮窗布局
    ├── components/
    │   ├── ui-primitives.css
    │   └── status-indicator.css
    └── pages/
        ├── home.css
        ├── history.css
        └── settings.css
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

## 编码准则

本节存在的原因是：LLM 写代码时会犯一些可以预见的错误。不是随机的错误，而是同样几个，一遍又一遍。我见过太多次，所以把它们写下来。

这些不是建议，是规则。照着做，产出的代码就不必返工；不当回事，产出的代码乍看惊艳，上线就崩。

### 1. 写代码前先读懂

LLM 产出烂代码最大的根源，就是写新代码之前没有读懂现有代码库。你看到一个任务，匹配到训练数据里的某个模式，就开始生成。这几乎总是错的。

写任何东西之前：

- 把你要改的文件读一遍。不是略读，是读。
- 看看项目里别处是怎么做类似事情的。如果有 API 路由的写法范式，就照着来；如果有个工具函数已经做了一半你需要的事，就用它。
- 看文件顶部的 import。它们告诉你这个项目实际在用什么库。项目到处用 fetch，就别引入 axios；项目用原生方法，就别引入 lodash。
- 看测试文件。它们告诉你预期行为到底是什么，而不是你以为该是什么。

这里的失败模式很清楚：你生成"正确"的代码，但它跟所在的代码库格格不入。能跑，但看起来像另一个人写的（因为确实是另一个实体写的）。然后人要么重写它以贴合项目风格，要么永远忍受不一致。两种都很糟。

如果你不确定这个项目里某件事是怎么做的，就说出来。"我在代码库里没看到 X 的范式，是该照 Y 的做法来，还是另起炉灶？"永远比瞎猜强。

### 2. 动手前先想清楚

没想清楚到底要做什么之前，别开始写代码。这听起来废话，但最常见的失败模式就是这个。

实际中长这样：

**把假设说出来。** 用户说"加个鉴权"，可能指 session cookie、JWT、OAuth、basic auth，或者其他五种东西。别默默选一个。说"我假设你要的是基于 JWT 的鉴权，带 refresh token，存在 httpOnly cookie 里。如果你想要别的，告诉我。"猜错了，你损失 10 秒；默默猜错了，你损失一小时。

**点明取舍。** 几乎每个实现选择都有代价。如果你要加缓存，就说"这拿内存换速度，并且引入了缓存失效这件我们此后得操心的事。"用户可能说"其实我不要这个复杂度。"写 200 行之前知道这点更好。

**存在多种做法时，简要地列出来。** 不是五种。两种，顶多三种。带上推荐。"这事有两种做法。A 更简单，但处理不了边界情况 X。B 全 cover，但引入对 Z 的依赖。除非你预期 X 真会发生，否则我选 A。"

**有搞不懂的地方，停下。** 别用听起来像那么回事的代码去填糊涂。在没搞懂需求时生成的代码，结果就是能糊弄过随便的 review，却在关键时候掉链子。直接说哪里搞不懂，问。

### 3. 简洁

写解决问题所需的最少代码。不是你想象中理论上能解决问题的最少代码，而是此刻真正解决这个具体问题的最少代码。

过度工程的冲动很强。抵制它。下面是过度工程的实际样子：

**过早抽象。** 你要发一种邮件。你写了一个 EmailService 类，带策略模式，支持多家供应商、多种模板引擎、各种重试策略。用户要的只是 `sendWelcomeEmail(user)`。就写这个函数。以后真需要更多，他们会开口。

```python
# 差：你写成了这样
class EmailService:
    def __init__(self, provider: EmailProvider, template_engine: TemplateEngine):
        self.provider = provider
        self.template_engine = template_engine

    async def send(self, template: str, context: dict, recipient: str, **kwargs):
        rendered = self.template_engine.render(template, context)
        await self.provider.send(recipient, rendered, **kwargs)

# 好：你本该写成这样
async def send_welcome_email(user):
    body = f"Welcome {user.name}! Your account is ready."
    await send_email(to=user.email, subject="Welcome", body=body)
```

**投机式的错误处理。** 你为不可能发生的错误到处包 try/catch。你校验来自自己代码、上游已经校验过的输入。你对永远不为 null 的值加 null 检查。每一行错误处理都是别人得读懂的一行。只处理真正会发生的错误。

**没必要的可配置性。** 你把 batch size 做成参数。你把重试次数做成可配置。你为永远不会变的东西加环境变量。配置不是免费的。每个配置项都是某人要做的一个决定、要设对的一个值。在有真正的理由之前，硬编码。

**死灵活性。** 只有一个实现的接口。只有一个子类的抽象基类。只被一个类型实例化的泛型参数。这些东西有成本（认知开销、间接层、更多文件要翻），而在第二个实现真正出现之前零收益。

简洁的检验：把你的代码拿给不熟项目的人看。如果他得问"这干嘛要这么抽象？"而答案是"万一我们需要……"，那你过度工程了。"万一我们需要"不是需求，是对未来的猜测，而对未来的猜测通常是错的。

### 4. 精准改动

改现有代码时，diff 越小越好。你改的每一行都可能引入 bug、都得有人 review、还会永远留在 git blame 里。

规则：

**别动没让你动的东西。** 你在修函数 A 的 bug，注意到函数 B 的变量名很怪——别管。函数 C 的注释有个错别字——别管。import 顺序不合你意——别管。你的活是修函数 A 的 bug。

**贴合现有风格。** 文件用单引号，你就用单引号。文件用 `snake_case`，你就用 `snake_case`。文件没分号，你就别加分号。文件用 `var`（对，哪怕在 2025 年），你在新增代码里也用 `var`，除非用户让你现代化。文件内的一致性胜过你的个人偏好。

**收拾自己留下的，不收拾别人的。** 你的改动让某个 import 没用了，就删掉它。让某个变量没用了，就删掉它。让某个函数没用了，就删掉它。但仅当是你的改动导致的。既存的死代码不归你管，除非有人让你清理。

**别重新格式化。** 别对原本没用 prettier 的文件跑 prettier。别把 4 空格缩进改成 2 空格。别把原本不按字母序的 import 重排。重新格式化制造海量 diff，淹没你真正的改动，让 code review 很痛苦。

检验：看看你的 diff。能不能把每一行改动都直接对应到被要求的事上、给出交代？要是有哪一行是因为"既然都进来了，顺手……"，就撤掉它。

### 5. 验证

"能跑的代码"和"你以为能跑的代码"之间，差的就是测试。对这层差别你要偏执。

**修 bug 时先写测试。** 动手修之前，先写一个能复现 bug 的测试。跑它。看着它挂。然后修 bug。跑测试。看着它过。这不是可选项，也不是 TDD 教条。这是唯一能证明你确实修好了那个东西、而不是仅仅让症状消失的办法。

**改之前和改之后都跑一遍现有测试。** 如果测试改前过、改后挂，你弄坏了什么。这很显然。不那么显然的是：如果测试在你改之前就已经挂着，说出来。别默默忽略既存的失败，让你的改动替它们背锅。

**别为写测试而写测试。** 检查构造函数有没有设好属性的测试一文不值。检查你的校验是否真的拦住坏输入的测试才有价值。测行为，不测实现。测有意义的 case，不测琐碎的。

**写不了测试，就说明原因。** 有时架构让测试很难做。这是有用的信息。"我没法轻松测这个，因为数据库调用跟业务逻辑紧耦合在一起"——这是个信号，说明可能得重构。别默默跳过测试然后指望没事。

### 6. 目标驱动

每个任务在动手写代码前都该有清晰的成功标准。标准模糊，就把它变具体；变不出具体的，就问。

把模糊的任务转成可验证的：

- "加校验" → "拦掉邮箱缺失或非法的输入，返回 400 并说明哪里错了，为这两种情况都加测试"。
- "修 bug" → "写一个复现上报行为的测试，让它通过，确认现有测试仍通过"。
- "提升性能" → "先 profile，定位瓶颈，修那一个具体问题，再测一次"。

任何超过一步的活，执行前先说出计划：

```
计划：
1. 用 migration 加新的数据库列
2. 更新 model 包含新字段
3. 改 API endpoint 以接受并返回该字段
4. 为该字段加校验
5. 为新行为写测试
6. 跑全量测试套件检查回归
```

这么做有两个好处：让用户在你浪费时间实现之前就能逮到思路上的失误；逼你真正把步骤想过一遍，而不是一头扎进去边做边凑。

### 7. 调试

出了问题不工作时，别猜。调查。

**把错误信息读完。** 整条，包括 stack trace。LLM 有个坏毛病：看到一个错误，立刻基于错误类型生成一个"修复"，根本不读它到底说了什么。一个 TypeError 可能指一百种情况。信息和 stack trace 告诉你是哪一种。

**先复现。** 改任何东西之前，先确保你能复现这个问题。复现不了，就没法验证你的修复。"我觉得这应该能修好"不是调试，是赌博。

**一次只改一处。** 你改了三处然后 bug 没了，你不知道是哪一处修好的，也不知道另外两处有没有引入新 bug。改一处，测。再改一处，测。

**没搞懂根因之前，别加 workaround。** 一个值意外为 null，别光加个 null 检查就过去。搞清楚它为什么是 null。null 检查也许能防崩溃，但底下的 bug 还在，以后会换个样子冒出来。

**卡住了，就说。** "我试了 X 和 Y 都没用。我看到的是这些。我觉得问题可能在 Z，但没把握。"这比默默瞎试 20 轮有用无穷倍。

### 8. 依赖

加依赖之前先想想。

你加的每一个依赖，都是一段你不掌控的代码，却要永久成为项目的一部分。它得被维护、更新、审计安全问题、被团队里每个人理解。代价几乎总比看上去高。

加一个包之前：

- 用项目里已有的东西能不能做？项目有 axios，就别加 node-fetch；项目用 date-fns，就别加 moment。
- 用标准库能不能做？`Array.prototype.map` 不需要 lodash；`crypto.randomUUID()` 存在的话就不需要 uuid。
- 这个依赖真的还在维护吗？看最近提交日期、看 issue 数量、看维护者回不回 issue。
- 它多大？为了格式化日期加个 500KB 的包，多半不值。

真要加依赖时，说明原因。"我加 zod，因为这个项目需要运行时 schema 校验，而现有依赖里没有干这个的"——可以。默默往 package.json 塞包——不行。

### 9. 沟通

你怎么就代码沟通，跟代码本身一样重要。

**说你做了什么、为什么。** 别光甩一段代码。"我把校验逻辑抽到单独的函数里，因为它在三个 endpoint 里重复了。这也让它能独立测试。"这样用户不用逐行读就懂了这次改动。

**标出顾虑。** 你实现了被要求的事，但你觉得这个路子有问题——说出来。"这个能跑，但它对列表里每一项都打一次数据库。列表一大就会慢。要不要我改成批量？"这种主动沟通能在以后省下几个小时。

**精确说出你不确定的是什么。** "我不确定这个库支不支持流式响应"——有用。"我觉得这应该能行"——没用。差别在于前者让用户清楚该去验证什么。

**别解释用户已经知道的事。** 他让你加个 REST endpoint，别解释 REST 是什么。他要个数据库索引，别解释索引干嘛。把解释的层次对齐到用户展现出来的知识水平。

**commit message 要具体。** "Fix bug"毫无用处。"修好用户查询里的空指针，当邮箱含大写字符时"才能让下一个人清楚发生了什么。

### 10. 常见失败模式

这些是我最常看到的模式。如果你逮住自己在干其中任何一件，停下来重新想想。

**厨房水槽。** 让你加一个功能，你"顺手"重构半个代码库。别。做那一件事。

**错误的抽象。** 你为一个只在一处存在的问题，造了一个漂亮的通用方案。重复远比错误的抽象便宜。先 copy-paste 两次，再谈抽象。

**隐形决策。** 你做了一个架构选择（数据库 schema、API 形状、鉴权策略），却没有把它作为一项决策标出来。这些选择难以撤销，用户应当知道你做了它。

**乐观路径。** 你写的代码把 happy path 处理得完美，对其他一切要么忽略要么崩溃。想想 API 返回 500 时会怎样。文件不存在时。用户提交空表单时。

**知识幻觉。** 你自信地用一个并不存在的 API、一个两个版本前就被移除的参数、或一个想象出来的库特性。如果你不是 100% 确定某个方法以这个确切签名存在，就说出来。查文档。看项目里的真实源码。

**风格漂移。** 你用自己"偏好"的风格写代码，而不是贴合项目。在 OOP 代码库里写函数式。在函数式代码库里写类。在 JavaScript 项目里写 TypeScript 范式。贴合代码库，不是贴合你的偏好。

**失控重构。** 你开始修一处。它碰到另一处。那处又碰到另一处。二十分钟后你改了 15 个文件，不确定自己最初要干什么。如果修复开始级联，停下。告诉用户发生了什么。继续之前先取得同意。

这些准则起作用的标志是：diff 里不必要改动更少、因过度复杂而返工更少、澄清问题发生在实现之前而不是犯错之后。
