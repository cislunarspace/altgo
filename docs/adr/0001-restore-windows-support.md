# ADR-0001：恢复 Windows 支持为主流平台

**日期**：2026-06-12
**状态**：已接受
**取代**：（原文件已删除，即 docs/superpowers/specs/2026-04-23-drop-windows-support-design.md）

## 背景

altgo 最初是同时支持 Linux 与 Windows 的跨平台应用。2026 年 4 月，为减轻维护负担并简化代码库，移除了 Windows 支持。原 Windows 实现用 PowerShell 子进程做按键监听、剪贴板与通知，带来安全问题（PowerShell 注入）与可靠性问题（依赖 Windows 执行策略、受杀毒软件干扰）。

恢复 Windows 支持的决定，来自用户把 Windows 作为一流目标平台的需求，以及长期维护的承诺。

## 决策

我们将把 Windows 恢复为**一流目标平台**，用**原生 Windows API** 替代此前移除的 PowerShell 子进程方案。

### 目标平台

- **操作系统**：Windows 10 1809+ / Windows 11
- **架构**：仅 x86_64（ARM64 延后）
- **打包**：经 Tauri bundler 产出 MSI

### 技术选型

| 组件 | Linux | Windows |
|-----------|-------|---------|
| 按键监听 | `xinput test-xi2` + `evtest`（子进程） | `WH_KEYBOARD_LL`（Win32 低级键盘钩子） |
| 音频录音 | `parecord`（子进程） | `cpal` crate（原生 Rust） |
| 剪贴板 | `xclip`/`xsel`/`wl-copy`（子进程） | `arboard` crate（原生 Rust） |
| 通知 | 无（结果展示统一走 Tauri overlay） | 无（Tauri overlay） |
| 浮窗定位 | Tauri overlay 定位 | `GetMonitorInfoW`（Win32 API） |
| 激活键捕获 | `evtest` + evdev | 捕获模式下的 `WH_KEYBOARD_LL` 钩子 |
| 显示器几何 | Tauri overlay 定位 | `MonitorFromWindow` + `GetMonitorInfoW` |

### 配置

- 按键监听配置新增字段 `windows_vk: Option<i32>`（存储 Windows 虚拟键码）
- 既有 `key_name` 字段作为跨平台配置可移植性的回退
- 配置/历史路径：`%APPDATA%\altgo\config.toml` 与 `%APPDATA%\altgo\history.json`
- 模型路径：`%LOCALAPPDATA%\altgo\models\`

### 二进制依赖

- `whisper-cli.exe` 经 `packaging/scripts/download-deps-windows.ps1` 下载
- 版本经 `packaging/scripts/versions.sh` 统一

### CI/CD

- PR/push：Linux（fmt + clippy + `cargo test --lib`）+ windows-check（fmt + clippy + `cargo test --lib -- --skip tauri_sink`；tauri_sink 测试因 tao 主线程限制无法运行，精确跳过）
- Tag push（release）：Linux（deb / rpm / flatpak）+ Windows MSI
- 另有 whisper-prebuild workflow：预编译 whisper 产物到独立 release（PR #106/#107）

### 测试策略

- 把平台 API 抽象到 trait 后面，便于单元测试
- 单元测试覆盖：事件转换、VK 映射、几何计算、配置往返、错误处理
- 在 Windows 运行器上做集成冒烟测试
- E2E 延到第二阶段

## 后果

- **取代** 2026 年 4 月的「移除 Windows 支持」ADR，后者现已成为历史
- 引入 `windows` crate 作为仅限 Windows 的依赖，用于 Win32 FFI
- 引入 `cpal`（仅 Windows）与 `arboard`（跨平台）；不引入通知库——Windows 结果由 Tauri overlay 展示
- 需要 `#[cfg(target_os = "windows")]` 守卫与平台特定模块文件
- Windows 二进制依赖需要下载脚本与 CI 缓存
- Linux 代码保持不变，无行为回归

## 备选方案

- **基于子进程的 Windows 支持**（已否决）：会重新引入 PowerShell 注入风险与促成原移除的可靠性问题。
- **剪贴板/通知的 Tauri 插件**（已否决）：对平台行为的控制不足，并与 Tauri 插件 API 紧耦合。
- **直接 WASAPI 录音**（已否决）：`cpal` 为 altgo 简单的录音需求提供了足够抽象，无需原生 COM/unsafe 的复杂度。
