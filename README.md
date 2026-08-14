# altgo

[![CI](https://github.com/cislunarspace/altgo/actions/workflows/ci.yml/badge.svg)](https://github.com/cislunarspace/altgo/actions/workflows/ci.yml)
[![文档](https://img.shields.io/badge/docs-online-2f6feb)](https://cislunarspace.github.io/altgo/)
[![Release](https://img.shields.io/github/v/release/cislunarspace/altgo)](https://github.com/cislunarspace/altgo/releases)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**altgo** 是一款跨平台桌面语音转文字工具。按住触发键说话，松开后自动完成录音、转写和可选润色，结果写入系统剪贴板并显示在悬浮窗中。

支持 **Linux**（Ubuntu 20.04+）和 **Windows**。目前不支持 macOS。

- [在线文档](https://cislunarspace.github.io/altgo/)
- [下载 Releases](https://github.com/cislunarspace/altgo/releases)
- [报告问题](https://github.com/cislunarspace/altgo/issues)

## 功能

- 长按右 Alt 录音，松开后自动转写
- 双击右 Alt 进入连续录音，再次单击停止
- 本地 `whisper.cpp` 转写，模型可在设置页下载和管理
- 可选常驻 `whisper-server`，模型只加载一次；不可用时自动回退到 `whisper-cli`
- 支持 OpenAI 兼容的 Whisper API 和小米 MiMo ASR 云端转写
- 支持 OpenAI 兼容 API 与 Anthropic Messages API 的 LLM 润色
- 结果写入剪贴板，并在悬浮窗展示，可再次复制
- 托盘常驻，可隐藏主窗口或退出应用
- 保存本地转写历史，可查看、复制、删除、清空和再次润色
- 只保存文本，不保存音频

## 安装

### Linux

安装前先将当前用户加入 `input` 组，否则无法读取键盘设备。执行后需要重新登录：

```bash
sudo usermod -aG input "$USER"
```

然后：

1. 从 [Releases](https://github.com/cislunarspace/altgo/releases) 下载对应安装包：`.deb`、`.rpm` 或 `.flatpak`。
2. 安装下载的包，例如：

   ```bash
   sudo apt install ./altgo_*.deb
   # 或
   sudo dnf install ./altgo-*.rpm
   # 或
   flatpak install --user ./altgo-*.flatpak
   ```

3. 重新登录后启动 altgo，在设置页完成转写配置。

官方 Linux 安装包会捆绑 `whisper-cli`。`.deb` 会声明桌面、音频、剪贴板、通知和 `evtest` 等依赖；`.rpm` 会声明主要桌面和音频依赖，若使用 Wayland，请确认系统已安装 `evtest` 且当前用户能读取 `/dev/input/event*`。

### Windows

1. 从 [Releases](https://github.com/cislunarspace/altgo/releases) 下载 `.msi` 安装包。
2. 双击安装包并按向导完成安装。
3. 启动 altgo，在设置页完成转写配置。

Windows 使用全局键盘钩子和系统默认麦克风，不需要 `input` 组或额外命令行工具。MSI 会自动处理 WebView2 Runtime。

## 快速开始

启动应用后，在 **设置** 页完成以下配置：

1. 选择转写引擎：本地模型、Whisper API 或 MiMo ASR。
2. 本地模式下载并选择一个模型；云端模式填写 API Key、服务地址和模型名。
3. 按需设置润色级别和润色服务。
4. 确认触发键，默认是右 Alt。
5. 点击保存。

默认使用长按模式：

```text
按下右 Alt → 开始录音 → 松开右 Alt → 转写 → 润色（可选）→ 剪贴板 + 悬浮窗
```

较长的发言可以双击右 Alt 进入连续录音，再单击一次停止：

```text
双击右 Alt → 连续录音 → 单击右 Alt → 转写 → 润色（可选）→ 剪贴板 + 悬浮窗
```

历史记录默认保存在：

```text
Linux:   ~/.config/altgo/history.json
Windows: %APPDATA%/altgo/history.json
```

## 配置

日常使用推荐通过设置页配置。需要手动编辑时，配置文件位于：

```text
Linux:   ~/.config/altgo/altgo.toml
Windows: %APPDATA%/altgo/altgo.toml
```

完整字段说明见 [`configs/altgo.toml`](configs/altgo.toml)。在线配置说明见[配置指南](https://cislunarspace.github.io/altgo/docs/configuration)。

### 转写引擎

本地转写是默认方式：

```toml
[transcriber]
engine = "local"
model = ""              # 在设置页下载模型后选择，或填写 GGML 模型路径
language = "zh"
```

OpenAI 兼容 Whisper API：

```toml
[transcriber]
engine = "api"
api_key = "sk-your-key"
api_base_url = "https://api.openai.com"
model = "whisper-1"
language = "zh"
```

小米 MiMo ASR：

```toml
[transcriber]
engine = "mimo"
api_key = "your-api-key"
api_base_url = "https://api.xiaomimimo.com/v1"
language = "zh"
```

### 润色

润色默认关闭。启用时需要配置协议、API 地址、模型和密钥：

```toml
[polisher]
level = "medium"        # none / light / medium / heavy
protocol = "openai"     # openai / anthropic
api_key = "sk-your-key"
api_base_url = "https://api.example.com"
model = "your-model"
```

也可以用环境变量覆盖 API Key：

```bash
export ALTGO_TRANSCRIBER_API_KEY="your-transcriber-key"
export ALTGO_POLISHER_API_KEY="your-polisher-key"
```

日志级别可通过 `RUST_LOG` 调整：

```bash
RUST_LOG=altgo=debug altgo
```

## 故障排查

### 按键没有反应

- Linux 确认当前用户已加入 `input` 组，并已重新登录。
- Wayland 确认 `evtest` 已安装，且当前用户能读取 `/dev/input/event*`。
- X11 确认 `xinput` 和 `xmodmap` 可用。
- 在设置页重新使用“按下以设置”捕获触发键。
- 用 `RUST_LOG=altgo=debug altgo` 查看键盘监听和流水线日志。

### 能录音但没有转写结果

- 先确认录音结束后悬浮窗是否进入“处理中”。
- 本地模式确认已下载模型，并且 `whisper-cli` 可用。
- API 模式确认 API Key、服务地址和模型名正确。
- MiMo 模式确认使用了完整地址 `https://api.xiaomimimo.com/v1`。
- 检查日志中的 `transcription failed`、模型路径和 API 返回错误。

### 没有写入剪贴板

- Linux 确认 `xclip`、`xsel` 或 `wl-copy` 至少有一个可用。
- 即使剪贴板写入失败，结果仍可在悬浮窗或历史页中复制。

## 开发

### 环境要求

- Rust stable
- Node.js 18+，推荐 Node.js 20+
- Tauri CLI：`cargo install tauri-cli --version "^2" --locked`
- Linux 或 Windows；完整 Tauri 构建需要满足 [Tauri 2 前置条件](https://tauri.app/start/prerequisites/)

### Linux 构建

```bash
git clone https://github.com/cislunarspace/altgo.git
cd altgo
cd frontend && npm install && cd ..
make deps-linux
make build
```

`make build` 会准备前端依赖和 `whisper.cpp` 相关二进制，然后执行 Tauri 生产构建。只想快速运行开发版时，可以使用：

```bash
cd frontend && npm install && cd ..
cargo tauri dev
```

### Windows 构建

在 PowerShell 中执行：

```powershell
.\build.ps1
```

也可以使用 `build.cmd`，或直接运行 `pwsh packaging/scripts/build.ps1`。需要 Rust 的 MSVC 工具链、Node.js 和 PowerShell 7+。

### 测试与检查

```bash
make test
make fmt
make lint
cd frontend && npm test
cd frontend && npm run build
```

对应的底层命令和贡献流程见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 项目结构

```text
frontend/             React 前端与 Tauri 页面
src-tauri/src/        Rust 核心、Tauri 命令和平台适配
resources/prompts/    润色 prompt 模板
configs/              配置模板
packaging/            Linux / Windows 打包脚本
docs-site/            面向用户的 Docusaurus 文档站
docs/                 面向维护者的设计与计划归档
```

核心流水线：

```text
按键事件 → 状态机 → 录音 → 转写 → 润色（可选）→ 剪贴板 + 悬浮窗 + 历史
```

核心 Rust 模块包括：

| 模块 | 职责 |
| --- | --- |
| `state_machine` | 长按、短按、双击和连续录音状态管理 |
| `key_listener` | Linux / Windows 全局按键监听 |
| `recorder` | Linux PulseAudio 与 Windows WASAPI 音频采集 |
| `transcriber` | 本地 whisper.cpp、Whisper API、MiMo ASR |
| `polisher` | OpenAI 兼容或 Anthropic 协议润色 |
| `voice_pipeline` | 编排录音、转写、润色和输出事件 |
| `history` | 本地历史记录读写 |
| `overlay` | 悬浮窗状态和窗口管理 |

## 相关文档

- [在线文档](https://cislunarspace.github.io/altgo/)：快速开始、配置、使用说明、架构和 FAQ
- [`CONTRIBUTING.md`](CONTRIBUTING.md)：开发流程、测试、CI、Release 和文档站部署
- [`docs/README.md`](docs/README.md)：设计与计划类文档索引
- [`CLAUDE.md`](CLAUDE.md)：面向维护者与 AI 协作者的架构速览
- [`CHANGELOG.md`](CHANGELOG.md)：版本变更记录

## 许可证

[MIT License](LICENSE)
