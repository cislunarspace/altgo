# Changelog

## Unreleased

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
