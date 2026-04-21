# 贡献指南

感谢你对 altgo 的关注！

本项目的**主要目标用户与环境是 Linux**；维护者目前仅在 **Ubuntu 20.04** 上做过完整验证。合并前请尽量在该环境或兼容配置下自测；Windows 为附带支持。

## 开发环境

- Rust **1.80+**（推荐最新稳定版，需满足 [Tauri 2 前置条件](https://tauri.app/start/prerequisites/)）
- **Node.js 18+**（建议 20+；前端使用 npm）
- Linux（推荐）/ Windows

### 平台特定依赖

- **Linux**：`xinput`、`xmodmap`、`parecord`、`xclip` 或 `wl-copy`、`notify-send`；Wayland 下按键监听还需 `evtest`，且需能读取 `/dev/input/event*`（常见：`sudo usermod -aG input $USER` 后重新登录）。完整 GUI 构建需 GTK/WebKit 等开发库，见 [README.md](README.md)「开发」。
- **Windows**：`ffmpeg`、PowerShell。从源码完整打包可与仓库一致地运行 `make deps-windows`（或 `pwsh packaging/scripts/download-deps.ps1`）下载捆绑二进制。

## 开发流程

1. Fork 仓库
2. 创建功能分支 (`git checkout -b feat/my-feature`)
3. 编写代码和测试
4. 确保通过检查：
   ```bash
   cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check
   cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings
   cargo test --manifest-path=src-tauri/Cargo.toml
   cd frontend && npm run build
   ```
5. 提交变更 (`git commit`)
6. 推送分支 (`git push origin feat/my-feature`)
7. 创建 Pull Request

## 提交消息格式

```
type: 简短描述

可选正文说明
```

类型：`feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`

## 代码风格

- 运行 `cargo fmt` 格式化代码
- `cargo clippy -- -D warnings` 零警告
- 公开 API 添加文档注释
- 函数 < 50 行，文件 < 1000 行

## 测试

- 新功能尽量附带单元测试或 HTTP 级模拟测试（与 `transcriber`/`polisher` 类似）
- 使用 `#[cfg(test)]` 模块组织单元测试
- 集成测试放在 `tests/` 目录

## 跨平台开发

添加平台特定代码时：

- 使用 `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "windows")]`
- 在对应平台的模块文件中实现（如 `key_listener/linux.rs`）
- 确保 `mod.rs` 导出统一的公共接口
- 尽可能使用子进程调用系统工具，避免 FFI

## 问题反馈

- 使用 GitHub Issues 报告 bug 或提出功能请求
- 包含：平台、版本、复现步骤、日志输出
