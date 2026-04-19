# altgo

**无需打字，言出法随** — 跨平台语音转文字桌面工具

按住右 Alt 键说话，松开后自动将语音转为文字、润色、写入剪贴板。

## 功能

- **长按触发**：长按右 Alt 键进入录音模式，松开自动停止并处理
- **双击切换**：双击右 Alt 键进入连续录音模式，再次单击停止
- **ASR 转写**：支持 OpenAI Whisper API 和本地 whisper.cpp 两种引擎
- **LLM 润色**：通过 OpenAI 兼容 API 对转写文本进行润色（支持 light/medium/heavy 三档）
- **剪贴板输出**：自动将结果写入系统剪贴板
- **桌面通知**：处理完成后弹出通知提示

## 平台支持

| 平台 | 按键监听 | 录音 | 剪贴板 | 通知 |
|------|---------|------|--------|------|
| Linux (X11/Wayland) | xinput | parecord | xclip/xsel/wl-copy | notify-send |
| macOS | CGEvent tap | sox | pbcopy | osascript |
| Windows | PowerShell hook | ffmpeg | clip.exe | PowerShell toast |

## 系统托盘

启动后会在系统托盘显示图标，点击图标可显示/隐藏主窗口，右键菜单提供"显示窗口"和"退出"选项。

## 安装

### Windows

1. 前往 [Releases](../../releases) 下载最新版 MSI 安装包
2. 双击安装，完成后在开始菜单找到 altgo
3. 安装音频工具：`winget install ffmpeg`
4. [配置语音识别和润色](#配置)

> 也可以下载 zip 包，解压后直接运行 `altgo.exe`。

### Linux (Debian/Ubuntu)

1. 前往 [Releases](../../releases) 下载对应架构的 `.deb` 包
2. 安装：`sudo dpkg -i altgo_*.deb`
3. 安装系统依赖：`sudo apt install xinput xdotool pulseaudio-utils xclip libnotify-bin`
4. [配置语音识别和润色](#配置)

### Linux (AppImage)

1. 前往 [Releases](../../releases) 下载最新的 `.AppImage` 文件
2. 赋予执行权限：`chmod +x altgo_*.AppImage`
3. 直接运行：`./altgo_*.AppImage`
4. 或使用 [AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher) 集成到桌面菜单

### macOS

1. 前往 [Releases](../../releases) 下载 tar.gz 包（支持 Apple Silicon 和 Intel）
2. 解压并放入 PATH：`sudo cp altgo /usr/local/bin/`
3. 安装音频工具：`brew install sox`
4. 在 **系统设置 → 隐私与安全性 → 辅助功能** 中授权终端或 altgo
5. [配置语音识别和润色](#配置)

## 配置

配置文件位置：

- **Linux**：`~/.config/altgo/altgo.toml`
- **macOS**：`~/.config/altgo/altgo.toml`
- **Windows**：`%APPDATA%\altgo\altgo.toml`

首次使用需创建配置文件，从[模板](configs/altgo.toml)复制即可。

### 语音识别（二选一）

#### 方式 A：本地 whisper.cpp（推荐，无需 API 密钥）

```toml
[transcriber]
engine = "local"
model = "~/models/ggml-base.bin"   # 模型文件路径
whisper_path = ""                   # 留空自动在 PATH 中查找 whisper-cli
language = "zh"
timeout_seconds = 30                # 请求超时时间 (s)
```

需要单独安装 [whisper.cpp](https://github.com/ggerganov/whisper.cpp) 并下载模型文件。

#### 方式 B：云端 Whisper API

```toml
[transcriber]
engine = "api"
api_key = "sk-your-key"
api_base_url = "https://api.openai.com"
model = "whisper-1"
language = "zh"
timeout_seconds = 30
temperature = 0.0               # Whisper API temperature (0.0-1.0)
prompt = ""                      # 提示词，提供领域词汇以提升识别率
```

### 润色（可选）

```toml
[polisher]
protocol = "openai"            # "openai" 或 "anthropic"
api_key = "sk-your-key"
api_base_url = "https://api.openai.com"
model = "gpt-3.5-turbo"
level = "medium"               # "none" / "light" / "medium" / "heavy"
max_tokens = 1024              # LLM 响应的最大 token 数
timeout_seconds = 60           # 请求超时时间 (s)
temperature = 0.3              # LLM temperature (0.0-2.0)
system_prompt = ""             # 自定义 system prompt，为空则使用内置 prompt
```

| 级别 | 效果 | 需要 API 密钥 |
|------|------|--------------|
| `none` | 不润色，直接输出转写文本 | 否 |
| `light` | 修正标点和错别字 | 是 |
| `medium` | 修正语法、改善语序通顺度 | 是 |
| `heavy` | 结构化重写，适合正式文档 | 是 |

### 环境变量

| 变量 | 说明 |
|------|------|
| `ALTGO_TRANSCRIBER_API_KEY` | 覆盖 `transcriber.api_key` |
| `ALTGO_POLISHER_API_KEY` | 覆盖 `polisher.api_key` |
| `RUST_LOG` | 日志级别，如 `altgo=debug` |

## 使用

1. 启动 altgo：`altgo`（或双击桌面快捷方式）
2. **长按右 Alt** → 开始录音 → 松开停止 → 自动处理 → 文字写入剪贴板
3. **双击右 Alt** → 连续录音 → 再次单击停止 → 自动处理

## 配置详解

所有字段都有默认值，完整模板见 [`configs/altgo.toml`](configs/altgo.toml)。

```toml
[key_listener]
key_name = "ISO_Level3_Shift"      # 触发键，默认右 Alt
long_press_threshold_ms = 200      # 长按判定阈值 (ms)
double_click_interval_ms = 300     # 双击判定间隔 (ms)
debounce_window_ms = 100           # 防抖窗口 (ms)，过滤 Windows 中文输入法抖动
poll_interval_ms = 30              # Windows 轮询间隔 (ms)
min_press_duration_ms = 100        # 最短按下时长 (ms)，过滤 IME 抖动

[recorder]
sample_rate = 16000                # 采样率
channels = 1                       # 声道数

[output]
enable_notify = true               # 是否显示桌面通知
notify_timeout_ms = 5000           # 通知显示时长 (ms)
inject_at_cursor = true            # 尝试将文本注入到当前光标位置（仅 Windows）
prefer_polished = true             # 注入/复制时优先使用润色后的文本

[logging]
level = "info"                     # "debug" / "info" / "warn" / "error"
```

## 架构

```
按键事件 → 状态机 → 录音 → ASR 转写 → LLM 润色 → 剪贴板 + 通知
```

altgo 是基于 Tauri 的桌面应用，前端使用 React，后端使用 Rust。

## 开发

### 前置依赖

- Rust 1.75+
- Node.js（前端开发需要）
- Tauri CLI：`cargo install tauri-cli --version "^2"`

### 构建

```bash
# Rust 后端
cargo build --release --manifest-path=src-tauri/Cargo.toml
cargo test --manifest-path=src-tauri/Cargo.toml
cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings

# Tauri GUI
cargo tauri dev               # 启动开发模式（前端 dev server + 桌面窗口）
cargo tauri build             # 生产构建

# 简化构建（使用 Make）
make build                    # 构建 + 复制 binary 到当前目录
make install                  # 安装到 /usr/local/bin + /etc/altgo/
```

首次启动前安装前端依赖：
```bash
cd frontend
npm install
```

## 许可证

[MIT](LICENSE)
