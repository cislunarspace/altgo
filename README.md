# altgo

[![CI](https://github.com/cislunarspace/altgo/actions/workflows/ci.yml/badge.svg)](https://github.com/cislunarspace/altgo/actions/workflows/ci.yml)
[![文档](https://img.shields.io/badge/docs-online-2f6feb)](https://cislunarspace.github.io/altgo/)
[![Release](https://img.shields.io/github/v/release/cislunarspace/altgo)](https://github.com/cislunarspace/altgo/releases)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

**altgo** 是一款 Linux 桌面语音转文字工具。按住触发键说话，松开后自动完成录音、转写和可选润色，结果写入系统剪贴板并显示在悬浮窗中。

支持 **Linux**（Ubuntu 20.04+）的 **x86_64** 与 **aarch64** 架构。目前不支持 Windows 和 macOS。

- [在线文档](https://cislunarspace.github.io/altgo/)
- [下载 Releases](https://github.com/cislunarspace/altgo/releases)
- [报告问题](https://github.com/cislunarspace/altgo/issues)

## 功能

- 长按右 Alt 录音，松开后自动转写
- 双击右 Alt 进入连续录音，再次单击停止
- 本地 SenseVoice 转写（内嵌 sherpa-onnx），模型只加载一次、响应快，可在设置页下载和管理
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

1. 从 [Releases](https://github.com/cislunarspace/altgo/releases) 下载对应安装包：`.deb` 或 `.rpm`。
2. 安装下载的包，例如：

   ```bash
   sudo apt install ./altgo_*.deb
   # 或
   sudo dnf install ./altgo-*.rpm
   ```

3. 重新登录后启动 altgo，在设置页完成转写配置。

`.deb` 和 `.rpm` 会声明桌面、音频、剪贴板、通知和 `evtest` 等依赖；若使用 Wayland，请确认系统已安装 `evtest` 且当前用户能读取 `/dev/input/event*`。

## 快速开始

启动应用后，在 **设置** 页完成以下配置：

1. 下载并选择本地 SenseVoice 模型。
2. 按需设置润色级别和润色服务。
3. 确认触发键，默认是右 Alt。
4. 点击保存。

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
~/.config/altgo/history.json
```

## 配置

日常使用推荐通过设置页配置。需要手动编辑时，配置文件位于：

```text
~/.config/altgo/altgo.toml
```

完整字段说明见 [`configs/altgo.toml`](configs/altgo.toml)。在线配置说明见[配置指南](https://cislunarspace.github.io/altgo/docs/configuration)。

### 转写引擎

本地 SenseVoice 是唯一转写方式：

```toml
[transcriber]
model = "sense-voice"   # 在设置页下载后自动填入；也可填写模型目录路径
language = "zh"          # 空字符串 = 自动检测（中/英/日/韩/粤）
threads = 0              # 0 = 自动取满 CPU 核数
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
- 确认已在设置页下载模型（SenseVoice）。
- 检查日志中的 `transcription failed`、模型路径和 API 返回错误。

### 没有写入剪贴板

- Linux 确认 `xclip`、`xsel` 或 `wl-copy` 至少有一个可用。
- 即使剪贴板写入失败，结果仍可在悬浮窗或历史页中复制。

## 开发

### 环境要求

- Rust stable
- Node.js 18+，推荐 Node.js 20+
- Tauri CLI：`cargo install tauri-cli --version "^2" --locked`
- Linux；完整 Tauri 构建需要满足 [Tauri 2 前置条件](https://tauri.app/start/prerequisites/)

### Linux 构建

```bash
git clone https://github.com/cislunarspace/altgo.git
cd altgo
cd frontend && npm install && cd ..
make build
```

`make build` 会准备前端依赖并执行 Tauri 生产构建。只想快速运行开发版时，可以使用：

```bash
cd frontend && npm install && cd ..
cargo tauri dev
```

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
packaging/            Linux 打包脚本
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
| `key_listener` | Linux 全局按键监听 |
| `recorder` | Linux PulseAudio 音频采集 |
| `transcriber` | 本地 SenseVoice（sherpa-onnx） |
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
