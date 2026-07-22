# Changelog

## v2.5.3 (2026-07-22)

### CI

- **新增 Windows Check job**：CI 在 windows-latest 跑 fmt + clippy + test（不打包），`cfg(windows)` 代码结束零覆盖（#98）。
- **修复 Windows 上 `cargo test` 无法运行**：rfd 静态导入 `TaskDialogIndirect` 需要 comctl32 v6 manifest，测试二进制此前启动即 0xc0000139；改为 build.rs 统一向 exe 与测试注入 manifest。
- **tauri_sink 测试平台 gate**：tao 要求 EventLoop 在主线程初始化，libtest 下各平台均无法运行，改为仅 Windows 编译并精确 skip；fixture 重做见 #104（#100）。
- **抽 `setup-linux-build` composite action**：CI 与 release 的 apt 依赖、`download-deps.sh`、tauri-cli 安装收敛到单点维护，CUDA 安装改为 `enable-cuda` 输入（#99）。
- **deploy-docs 改为 CI 成功后再部署**，避免红 CI 也发文档站（#97）。

## v2.5.2 (2026-07-21)

### Fixes

- **首次运行浮窗黑影和重叠**：overlay 窗口初始尺寸 (200×48) 与实际固定尺寸 (520×180) 不一致，首次 `set_state` 时 resize 导致透明窗口暴露新区域，合成器渲染出黑影且小窗口内容重叠。改 tauri.conf.json 初始尺寸对齐 `OVERLAY_SIZE`。

## v2.5.1 (2026-07-21)

### Packaging

- **移除 ffmpeg 依赖**：删除所有 ffmpeg 下载/打包/文档引用，`whisper-cli` 成为唯一的捆绑二进制。
- **CUDA runtime 不再随包分发**：改为用户自行安装 CUDA runtime 启用 GPU，未安装时自动回退 CPU，避免 deb 包膨胀到 1.5G。
- **`.so` 软链精简**：新增 `trim_so_to_soname()` 消除 Tauri 打包时 `.so` 三份复制膨胀（libggml-cuda 单份 131M → 避免 393M）。

### Docs

- **CONTEXT.md 术语扩充**：补充 Transcription Engine、MiMo ASR、Provider Preset、Model Catalog、Provider Category。

## v2.5.0 (2026-07-20)

### Features

- **MiMo ASR 语音识别后端**：新增 `engine="mimo"`，接入小米 MiMo-V2.5-ASR 云端 API，支持 wav/mp3、中英文自动检测；Settings 引擎切换新增「MiMo ASR (小米)」选项，选中自动填充 API 地址。
- **模型预设选择器**：转写和润色设置可按服务商预设快速填充——润色预设含 DeepSeek、Kimi、智谱、通义、OpenAI、Anthropic、SiliconFlow，语音识别预设含 MiMo ASR、OpenAI Whisper、本地 whisper.cpp；每个预设带推荐模型目录，支持搜索和展开详情。
- **whisper GPU 加速构建**：release 工作流安装 CUDA toolkit，本地 whisper 编译启用 GPU。

### Fixes

- **悬浮窗动画全面修复**：相位切换的闪烁、跳变、黑晕。窗口改固定尺寸（520×180），切换不再 resize，消除合成黑边与位移跳变；crossfade 只做淡出、不带位移；transitionend 只响应容器自身事件；先发结果文本再切 done，杜绝空 island 闪烁；hidden 延迟 220ms 再 hide，退出动画真正可见，并用代际计数防止竞态；去掉 box-shadow——半透明阴影叠在透明窗口上会被合成器预乘成黑影。

## v2.4.5 (2026-07-20)

### Refactor

- **架构审计修复（#84-#95）**：统一错误处理路径、类型重命名对齐领域语言、CSS 拆分为独立模块。
- **model 深模块化**（#66-#70）：Output 路径合并、五项架构优化，消除过度工程。
- **OverlaySink 解耦**：提取 trait 接口，TauriPipelineSink 与浮窗实现分离。
- **HistoryStore 注入**：消除对 Tauri 全局状态的依赖，构造时注入。
- **dead code 清理**：删除未使用的 notify 输出路径（#63）、三处死代码（#64）、废止的 PowerShell 脚本。

### Style

- **CSS token 统一**：transition 收敛、中文间距适配，样式系统规范化。

### Tests

- 补齐前端测试覆盖：StatusIndicator、useConfigForm、overlay 模块。
- 补齐 TauriPipelineSink 单元测试。

### Docs

- 更新 CLAUDE.md 反映 error.rs 和 voice_pipeline 新类型名。
- 同步写作规范至 CLAUDE.md 和 AGENTS.md。
- 精简 README，去除冗余结构。

## v2.4.4 (2026-06-19)

### Fixes

- **状态机回归修复**：按键事件路径重构后，`key_events` 通道关闭时管道循环不再永久挂起——现在会打印 `tracing::warn` 并干净退出。
- **PotentialPress 状态残留**：短按被 `min_press_duration` 拒绝后状态未回退到 Idle，后续系统重复 release 事件会让状态机从未真正按下就进入等待双击状态。现回退到 Idle。
- **转写失败状态卡死**：`transcriber.transcribe()` 返回错误时只调用 `on_error` 不恢复状态，前端状态指示器卡在 processing。现补充 `on_status_change("idle")`。
- **windows_vk 三态不一致**：`windows_vk` 与 `linux_evdev_code` 对齐，支持 JSON `null` 清除（之前 `{"windowsVk": null}` 被静默忽略）。
- **失效的 debounce_window 配置**：`debounce_task` 移除后 `debounce_window_ms` 成为死字段，现从配置和模板中彻底移除。

### Refactor — 深化模块（架构审查 5 项全部落地）

- **内联状态机**（#3）：按键事件路径从 4 层（thread → debounce_task → `Machine::run` → select）简化为 2 层（`key_events` → select 内联状态机），净减 139 行。
- **配置镜像结构体消除**（#1）：`config.rs` 直接存储 `Duration`（配合 serde `duration_ms`/`duration_secs` 辅助模块，TOML `_ms`/`_seconds` 字段名通过 alias 保持向后兼容）。移除 4 个模块的镜像 Config 结构体和 53 行机械复制的 `From` 实现。
- **TauriPipelineSink 解耦**（#2）：`HistoryStore` 改为构造时注入，`on_transcription_result` 不再运行时调用 `app.state()`。
- **voice_pipeline 拆分**（#5）：946 行单体文件拆为 5 个聚焦子模块（`sink`/`builder`/`context`/`handlers`/`mod`），公共接口通过 re-export 保持向后兼容。

### Tests

- 新增 `ConfigStore` 集成测试（7 个），覆盖 patch-validate-save 完整周期、windows_vk/linux_evdev_code 三态清除、无效配置拒绝。
- 测试总数从 146 提升到 156。

## v2.4.3 (2026-06-17)

### Features — Windows 平台支持

- **Windows 正式支持**：altgo 现可通过 MSI 安装运行于 Windows。录音用 cpal（WASAPI），剪贴板用 arboard，按键监听用 `WH_KEYBOARD_LL` 低级钩子，显示器几何用 `GetMonitorInfoW`。
- **MSI 打包与发布流水线**：`build.ps1` 等价于 Linux 的 `make build`；CI 在 tag 推送时自动构建 MSI 并发布到 GitHub Releases。
- **激活键捕获**：Windows 端 `key_capture` 通过 `WH_KEYBOARD_LL` 捕获激活键，与 Linux evdev 路径对齐。

### Refactor — 架构重构

- **Voice Pipeline 模块合并**（#46）：将 pipeline_orchestrator / context / command_handler / event_handler 合并为单一深模块 `voice_pipeline`，保留 `PipelineSink` 接缝。一条录音→转写流程现在集中在一个模块内。
- **HistoryStore 成为唯一接口**（#50）：游离的 `append_entry` 等函数收进 `HistoryStore`，新增 `count()` 领域方法。调用方不再直接处理文件路径。
- **trait 化注入点统一**：`KeyListener`、`Recorder`、`Output`、`Transcriber`、`Polisher` 均定义为 trait，pipeline 通过 `Box<dyn Trait>` 消费，测试可注入 fake。
- **ConfigPatch 移入 config.rs**：补丁应用逻辑与字段定义共处一处，新增配置字段只改一个文件。
- **Settings 拆分 hooks**：配置表单与模型管理逻辑分别抽到 `useConfigForm`、`useModelManager`。
- **错误边界类型化**：移除 `From<anyhow::Error>` 回退，各子系统边界返回类型化错误。

### Docs

- CLAUDE.md 与 `docs/agents` 文档翻译为中文，要求中文交流。
- 新增 Windows 支持的 ADR 与实现计划。

## v2.4.2 (2026-06-12)

### Fixes

- **设置保存修复**：Settings 页面保存配置时发送 patch payload，恢复配置保存 IPC 的参数结构。
- **录音态悬浮窗修复**：开始新录音时清空旧转写结果，并在显示窗口前同步 overlay 状态，避免上一次结果悬浮窗与录音中悬浮窗同时出现。

## v2.4.1 (2026-06-11)

### Fixes

- **悬浮窗黑影修复**：引入 `OverlayWindow` seam，把 Tauri 窗口操作封装到 adapter，显示前先完成尺寸、位置与原生窗口标志准备，减少 Linux 透明窗口首帧黑影。
- **Overlay 动画合成优化**：降低 overlay 阴影半径/透明度，改用不透明 solid surface，并把 opacity 动画从透明顶层窗口移动到内部 island，避免 alpha 叠加产生黑色晕影。
- **状态切换竞态修复**：取消未完成的 crossfade timer，避免 hidden 事件被旧 timer 覆盖后悬浮窗卡住显示。

## v2.4.0 (2026-06-11)

### Features

- **常驻 whisper-server 后端**：`engine="local"` 现在自动拉起常驻 whisper-server 进程，模型只载入一次内存，之后每句通过本地 HTTP 转写。告别每句重载 ~1.5–2.9 GB 模型的冷启动成本。
- **自动降级**：server 启动失败、端口冲突、运行期崩溃时自动回退到一次性 `whisper-cli`，旧安装也能工作。
- **调优旋钮**：新增 `transcriber.threads`（`0` = 自动取满 CPU 核数，whisper 默认仅 4）和 `transcriber.beam_size`（`<=1` = 贪心解码，最快）。

### Performance

- **~10× 转写提速**：medium 模型（1.5 GB）4 秒音频，每句从 ~2.8 s 降到 ~0.24 s（常驻后端 + GPU 加速叠加）。
- **CUDA 自动探测**：构建端检测到 `nvcc` 时自动启用 GPU 后端，捆绑 CUDA runtime 库。无 nvcc 时纯 CPU。
- **悬浮窗动画优化**：提取 `OverlayManager` 模块，单次 IPC 事件替代多次窗口操作；移除 `backdrop-filter` 实时模糊，改用纯色背景；进度条改用 `scaleX` 合成层动画，消除布局重排。

### Build

- **可移植 CPU 基线**：始终关闭 `-march=native`，避免旧机器 `SIGILL`。
- **whisper-server 捆绑**：`download-deps.sh` 现在同时捆绑 `whisper-server` 与 `whisper-cli`。

### Refactor

- **OverlayManager**：悬浮窗物理管理（尺寸/位置/显隐）从 `TauriPipelineSink` 中剥离为独立模块，接口从 3 个窗口方法缩减为 `set_state(OverlayState)` 单一意图调用。
- **CI 修复**：ffmpeg 下载源 `johnvansickle.com` 对 GitHub Actions 返回 415，新增 BtbN GitHub 镜像回退。

## v2.3.1 (2026-04-23)

### Packaging / CI

- Unified release pipeline: tag push now auto-builds **deb**, **rpm**, **AppImage**, **Flatpak**, and **AUR PKGBUILD**
- Added **RPM** bundle target for Fedora/RHEL/openSUSE
- Added **Flatpak** manifest (`.flatpak` artifact on GitHub Release)
- Added **AUR** PKGBUILD template and generator script
- Integrated AppImage build into `release.yml` (removed standalone `appimage.yml`)
- Unified version constants in `packaging/scripts/versions.sh` (fixed whisper.cpp v1.7.5 → v1.8.4 skew)

## v2.2.4 (2026-04-22)

### Packaging / CI

- Linux **deb** 在 **ubuntu-22.04** 上构建，链接 **glibc 2.35**，可在 **Ubuntu 22.04 (Jammy)** 等环境运行（避免在更新 runner 上出现 GLIBC_2.39+ 仅新系统可用的问题）
- Tauri 可执行文件统一为 **`altgo`**（与文档、桌面项、`make install` 一致；不再使用 `altgo-tauri` 作为安装名）

### Release

- GitHub Release 正文本版本起从 **CHANGELOG** 自动生成，并附与上一 tag 的对比链接

## v2.1.0 (2026-04-21)

### Features

- 转写历史：本地 `history.json` 持久化；History 页面列表、删除、清空、复制与单条再润色；管道成功后写入并广播 `history-updated`

### Improvements

- 更新应用图标与依赖；设置页与界面样式打磨

### Fixes

- 代码审查：并发、错误处理与资源管理等修复
- 构建与综合稳定性改进

## v2.0.1 (2026-04-21)

### Bug Fixes

- Windows：修复 `key_capture` 中 VK 字母显示名的类型错误（`i32` / `u8` / `char`），恢复 Release 构建

## v2.0.0 (2026-04-21)

### Documentation

- 全面更新 README：安装（Release、Makefile/deps、从源码轻量路径）、开发流程与 Makefile 目标说明、`key_listener` 可选字段与相关文档链接
- 更新 CLAUDE.md（`make build` 行为、`capture_activation_key`、`key_capture`、前端主题与样式结构）
- 更新 CONTRIBUTING.md（工具链版本、平台依赖、`frontend` 构建校验）
- 修复 docs-site 快速开始（移除不存在的安装脚本、修正构建命令与 MDX）；docs-site README 改用 npm
- README / docs-site：Linux 优先；标明 Ubuntu 20.04 测试环境；强调 **input 组** 必做；文档口径为仅本地 whisper.cpp、LLM 润色走 OpenAI 兼容 API、结果以**悬浮窗**为主；删除过时「平台支持」表；开发者以 **`make build`** 为主流程；最终用户用 deb/AppImage/MSI
- README / docs-site：默认配置方式改为**应用内设置**；手写 `altgo.toml` 标为高级；强调**预编译包捆绑** ffmpeg / whisper-cli，减少用户侧依赖清单

### CI / Release

- GitHub Actions：修复 CI 使用 `src-tauri/Cargo.toml`；CI/Release 在 Tauri 构建前执行 `download-deps.sh`；Release 增加 Windows MSI 与合并产物；Pages 在每次推送 `master` 时部署；AppImage 工作流注入 `VERSION`

## v1.0.0 (2026-04-14)

### Features

- Cross-platform desktop voice-to-text tool
- Hold right Alt key to record, release to transcribe
- Dual transcription backends: Whisper API (OpenAI-compatible) and local whisper.cpp
- LLM text polishing with 4 levels (none/light/medium/heavy)
- Automatic clipboard output
- Platform-native notifications
- GUI settings panel with real-time config reload

### Platform Support

- **Linux**: x86_64 and ARM64 builds with DEB packages
- **Windows**: x86_64 with MSI installer

### Bug Fixes

- Fix WiX v5 ComponentGroup configuration for MSI builds
- Fix release workflow runner compatibility
- Fix CJK font rendering in GUI panel
- Fix GUI config save guards
- Resolve multiple code quality and safety issues
