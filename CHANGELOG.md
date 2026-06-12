# Changelog

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

## Unreleased

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
