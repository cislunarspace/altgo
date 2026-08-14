# 领域术语表

本文件定义 altgo 代码库中使用的术语。写代码、文档和架构讨论时，请原样使用这些词。

## 核心流水线

**语音流水线（Voice Pipeline）**
端到端处理链：按键 → 录音 → 转写 → 润色 → 输出。由状态机驱动，运行时由 `PipelineController` 管理。

**转写引擎（Transcription Engine）**
把 WAV 音频转成文本的后端。三种实现：`ResidentWhisper`（本地 whisper.cpp，支持 GPU）、`WhisperApi`（OpenAI 兼容 HTTP API）、`MimoAsr`（小米 MiMo-V2.5-ASR 云 API）。通过 `TranscriberConfig` 的 `engine` 字段选择。

**MiMo ASR**
小米的云端语音识别服务。走 OpenAI 兼容的 `chat.completions` 端点，用 `input_audio` 内容类型。支持 wav/mp3，自动检测语言（zh/en），按标准聊天补全响应格式返回文本。端点：`https://api.xiaomimimo.com/v1`。

**提供商预设（Provider Preset）**
预配置的 API 提供商模板，包含名称、base URL、API 格式、推荐模型与分类。用于快速填充转写与润色引擎的设置。存放在 `frontend/src/config/modelPresets.ts`。

**模型目录（Model Catalog）**
与某提供商预设关联的推荐模型列表。每条含模型 ID、显示名、描述、上下文窗口与输入模态（文本/音频/图像）。用户可从目录中挑选，无需手动输入模型名。

**提供商分类（Provider Category）**
API 提供商的分类：`official`（OpenAI、Anthropic）、`cn_official`（DeepSeek、Kimi、智谱）、`mimo`（小米）、`aggregator`（SiliconFlow、OpenRouter）、`custom`（本地 SenseVoice）。决定预设选择器 UI 中的显示顺序与分组。

**管道状态（Pipeline Status）**
语音管道任意时刻的生命周期阶段：`Idle`、`Recording`、`Processing`、`Done`、`Stopped`。在 Rust 后端以 `PipelineStatus` 枚举表示，跨 IPC 边界序列化为小写字符串给前端。

**PipelineController**
拥有管道运行句柄与共享的 `PipelineStatus` Arc。负责 start、stop、restart。不知道如何生成管道——调用方注入 spawn 闭包，使本模块不依赖 Tauri 与 sink。位于 `pipeline_controller.rs`。

**PipelineSink**
接收运行中管道事件的 trait：状态变更、进度、错误、转写结果。`tauri_sink.rs` 的 `TauriPipelineSink` 是生产环境唯一的实体适配器。转写结果路径把业务工作（剪贴板写入 + 历史追加）委托给构造时注入的 `TranscriptionDispatch` trait 对象；sink 本身只发 Tauri 事件并切换浮窗状态。

## 配置

**ConfigStore**
在 `Mutex` 后持有内存中的活动配置，连同文件路径。暴露 `snapshot`、`snapshot_blocking`、`apply_patch`。所有配置变更都经 `apply_patch` 校验并写盘。校验失败时内存已部分应用、不落盘（非原子回滚）。位于 `config_store.rs`。

**ConfigPatch**
配置的部分更新：所有字段可选，未提供的字段保持不变。`linux_evdev_code` 字段用三态反序列化器区分缺省（不修改）与 JSON `null`（清除已存代码）。这是 `save_config` 经 IPC 接受的类型。

## 历史

**HistoryStore**
包装历史 JSON 文件，暴露具名操作：`list`、`count`、`append`、`delete`、`clear`、`get`、`update_text`。调用方从不接触文件路径或模块私有辅助函数。位于 `history.rs`。每个实例克隆便宜（只含一个 `PathBuf`）。

**历史条目（HistoryEntry）**
单条转写记录：`id`、`createdAtMs`、`rawText`（转写原文）、`text`（润色后，或与原文相同）。永不存音频。

## 输出

**悬浮窗（Overlay）**
录音、处理与结果显示时出现的悬浮状态窗口。定位在主显示器（Windows 用 `GetMonitorInfoW` 取显示器几何）。状态切换时由 `TauriPipelineSink` 经 `OverlaySink` 抽象管理；`TauriPipelineSink` 只描述意图（"recording" / "processing" / "hidden" / "done"），浮窗管理器把它转成窗口尺寸、位置、显示与隐藏。窗口在全部阶段使用一个固定尺寸——会话中调整透明窗口尺寸会在 Linux 合成器上产生黑色边缘，所以阶段切换只替换前端内容（CSS 交叉淡入）。`hidden` 先发出，实际隐藏延迟约 220ms 以便退出动画可见。转写结果路径上，`transcription-result` 在 `done` 浮窗状态之前发出，前端不会渲染出空 island。island 不使用 `box-shadow`——半透明阴影叠加在透明窗口上会在某些 Linux 合成器上合成出暗色光晕。

**润色器（Polisher）**
可选的 LLM 后处理步骤。由 `PolishLevel`（`none`/`light`/`medium`/`heavy`）控制。与任意 OpenAI 兼容聊天 API 或 Anthropic Messages API 通信。

**PromptStore**
管理润色器的 prompt 模板文件：从 `resources/prompts/` 加载，把 base + 各档后缀组合成完整系统 prompt，首次使用时校验。启动时加载一次，改文件需重启。校验失败时降级（退回自定义或内置 prompt，润色继续用原文）。

**Prompt 模板（Prompt Template）**
`resources/prompts/` 中的文本文件：`base.txt`（共享指令 + 中文写作指导）或 `{level}-suffix.txt`（追加到 base 之后的各档指令）。运行时组合产生发给 LLM 的完整系统 prompt。

**系统 prompt（System Prompt）**
发给 LLM 用于润色的完整 prompt 文本，组合自 `base.txt` + `{level}-suffix.txt`，缓存于内存，启动时加载一次。

## 录音

**录音输出格式（Recorder Output Format）**
语音管道期望录音器以 WAV 字节返回的音频格式。配置的 `sample_rate` 描述目标录音输出采样率（默认 16kHz）；输出总是单声道 16 位 PCM。平台录音器在可行处把原生设备格式适配成这一目标形态。

**Windows 录音格式适配（Windows Recording Format Adaptation）**
在 Windows 上，录音器从默认 WASAPI 输入设备捕获，把常见设备采样格式（`i16`、`u16`、`f32`）适配为有符号 16 位 PCM。目标输出为单声道时，多声道输入下混为单声道。设备无法以目标采样率捕获时，录音器在返回 WAV 字节前把音频重采样到目标采样率。

## 按键输入

**按键监听器（KeyListener）**
管道运行期间持续监听配置的激活键并发出 `KeyEvent` 的接口。由平台适配器实现（Linux 为 `X11Listener`，Windows 为 `WindowsListener`）。管道以 `Box<dyn KeyListener>` 消费它，两边平台运行同一套生命周期代码。
_Avoid_: key listener（指概念时小写）、platform listener。

**按键捕获（KeyCapture）**
设置配置期间一次性捕获任意物理键的**函数**（无 trait 抽象，按平台 cfg 分派；Linux 用 evtest，Windows 用 WH_KEYBOARD_LL hook），返回 `KeyListenerConfig` 所需的键标识符（`key_name`、`linux_evdev_code`、`windows_vk`）。平台实现复用与 `KeyListener` 相同的底层输入机制，但暴露同步阻塞式 API。
_Avoid_: capture mode、key capture mode。

**激活键（Activation Key）**
按住即开始录音的物理键。按设备配置为 X11 keysym 名（`key_name`）或 evdev 扫描码（`linux_evdev_code`）。Wayland 上优先 evdev 路径。在 Windows 上经 Windows 虚拟键码（`windows_vk`）配置；缺省时回退 `key_name`。若用户在 Windows 捕获后通过设置修改 `key_name`，`windows_vk` 会被清除，使新的 `key_name` 生效。

**Windows VK**
在 Windows 平台上标识激活键的 Windows 虚拟键码（`i32`）。配置中以 `windows_vk` 存储。捕获模式下经低级键盘钩子（`WH_KEYBOARD_LL`）在运行时捕获。在 Windows 上优先于 `key_name`，除非刚被手动编辑 `key_name` 清除。

**Windows VK 名称映射（Windows VK Name Map）**
从 X11 风格 keysym 名到 Windows 虚拟键码的小映射，用作 `windows_vk` 缺省时的回退。支持最常选作激活键的键：`Alt_L`、`Alt_R`、`Control_L`、`Control_R`、`Shift_L`、`Shift_R`、`space`、`Return`、`Tab`、`Escape`、`F1`–`F12`。未知名称使 `WindowsListener::new` 快速失败并给出清晰错误，用户能立即知道原因，而不是困惑为何按键不触发录音。

**Windows 捕获模式（Windows Capture Mode）**
用户在 Windows 激活键捕获期间按下键时，低级钩子返回 Windows 虚拟键码。响应以 `windows_vk` 存储，并给出 X11 风格 `key_name`（如 `Alt_R`），使显示的激活键跨平台一致。捕获实现在 `key_listener::windows`，经 `key_capture` 以 `CaptureActivationResponse` 重新导出。

**状态机（State Machine）**
把原始按键事件翻译成管道 `StartRecord` / `StopRecord` 命令的 5 状态 FSM（`Idle → PotentialPress → Recording → WaitSecondClick → ContinuousRecording`）。
