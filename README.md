# altgo

语音转文字桌面工具。按住右 Alt 键说话，松开即转写，结果写入剪贴板，并显示在悬浮窗中。

支持 **Linux**（Ubuntu 20.04+）和 **Windows**（MSI 安装包）。不支持 macOS。

**在线文档**：https://cislunarspace.github.io/altgo/（源码在 `docs-site/`）

## 功能

- 长按右 Alt 键录音，松开自动转写
- 双击右 Alt 键进入连续录音模式，再次单击停止
- 本地 whisper.cpp 转写，模型可在设置里下载。有 NVIDIA 显卡并安装 CUDA runtime 时自动启用 GPU 加速，否则回退 CPU。本地自动拉起常驻 whisper-server 提速，不可用时回退一次性 whisper-cli
- 可选 LLM 润色，支持 OpenAI 兼容与 Anthropic Messages 两种协议
- 结果写入剪贴板，并在悬浮窗展示，可再次复制
- 托盘常驻，可隐藏窗口或退出
- 转写历史保存在本地 history.json，可查看、复制、删除、清空、再次润色。历史只保存文本，不保存音频

## 系统要求

### Linux

- 需将当前用户加入 `input` 组，否则无法读取键盘设备。加入后须重新登录

  ```bash
  sudo usermod -aG input "$USER"
  ```

- 其余依赖由安装包自动处理

### Windows

- 需要 WebView2 Runtime（MSI 会自动安装），使用系统默认麦克风
- 无需额外依赖

## 安装

### Linux

1. 到 [Releases](https://github.com/cislunarspace/altgo/releases) 下载安装包
2. 按格式安装：

   ```bash
   sudo apt install ./altgo_*_amd64.deb     # deb
   sudo dnf install ./altgo-*.rpm           # rpm
   flatpak install --user ./altgo-*.flatpak # flatpak
   ```

3. 加入 `input` 组并重新登录（见系统要求）
4. 启动应用，在设置里完成转写模型与润色

### Windows

1. 到 [Releases](https://github.com/cislunarspace/altgo/releases) 下载 MSI
2. 双击运行，按向导完成安装
3. 启动应用，在设置里完成转写模型与润色

## 使用

启动后先在设置页完成转写模型、润色与触发键，然后：

1. 长按右 Alt 录音，松开后转写（可选润色），结果进剪贴板和悬浮窗
2. 双击右 Alt 进入连续录音，再次单击停止
3. 在历史记录页可浏览、复制、删除、清空、再次润色

默认触发键是右 Alt。按它没反应时，Linux 检查是否已加入 `input` 组并重新登录，Windows 确认 altgo 窗口在前台。可用 `RUST_LOG=altgo=debug altgo` 查看日志。

## 配置

配置文件位于 `~/.config/altgo/altgo.toml`，模板见 [`configs/altgo.toml`](configs/altgo.toml)。

| 变量 | 说明 |
| ---- | ---- |
| `ALTGO_POLISHER_API_KEY` | 覆盖润色 API 密钥 |
| `ALTGO_TRANSCRIBER_API_KEY` | 覆盖云端转写 API 密钥 |
| `RUST_LOG` | 日志级别，如 `altgo=debug` |

## 架构

```text
按键事件 → 状态机 → 录音 → whisper.cpp 转写 → 可选润色 → 剪贴板 + 悬浮窗 + 历史
```

基于 Tauri，前端 React，核心逻辑 Rust。关键模块：

| 模块 | 职责 |
|------|------|
| `state_machine` | 按键状态管理（长按 / 双击 / 连续录音） |
| `key_listener` | 按键监听（Linux xinput / Windows WH_KEYBOARD_LL） |
| `recorder` | 音频采集（Linux parecord / Windows cpal） |
| `transcriber` | 转写后端：本地 whisper-server（回退 whisper-cli）、OpenAI 兼容 API、小米 MiMo 云 API |
| `whisper_server` | 常驻 whisper-server 管理，模型常驻内存 |
| `polisher` | OpenAI 兼容或 Anthropic 协议润色 |
| `output` | 剪贴板写入（Linux xclip / xsel / wl-copy，Windows arboard） |
| `history` | 本地 history.json 读写 |
| `model` | GGML 模型下载与管理 |
| `overlay` | 悬浮窗（状态意图与窗口操作分离） |
| `tauri_sink` | 管道事件转 Tauri 事件与悬浮窗状态 |
| `pipeline_controller` | 管道生命周期与状态跟踪 |

## 开发环境

前置依赖：Rust stable、Node.js 18+、Tauri CLI（`cargo install tauri-cli`）。

从源码构建：

```bash
cd frontend && npm install && cd ..
make deps-linux
make build
```

Windows 用 `.\build.ps1` 或 `build.cmd`。系统依赖（GTK/WebKit 等）见 [CLAUDE.md](CLAUDE.md)。

常用命令：

| 命令 | 说明 |
| ---- | ---- |
| `make deps-linux` | 下载 whisper 依赖到 target/deps/bin |
| `make build` | 构建并拷贝依赖二进制到发布目录 |
| `make install` | 安装到系统（/usr/local/bin 等） |
| `make test` / `make fmt` / `make lint` | 测试、格式化、Clippy 检查 |
| `cargo tauri dev` | 开发模式热重载 |

## 相关文档

- 在线文档：https://cislunarspace.github.io/altgo/
- [CONTRIBUTING.md](CONTRIBUTING.md)（CI / Release / GitHub Pages 维护）
- [CLAUDE.md](CLAUDE.md)（面向 AI 与贡献者的架构速览）
- [docs/](docs/)（设计与计划归档）

## 许可证

[MIT](LICENSE)
