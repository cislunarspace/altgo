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
| macOS | CGEvent tap | sox/ffmpeg | pbcopy | osascript |
| Windows | PowerShell hook | ffmpeg/sox | clip.exe | PowerShell toast |

## 前置条件

### Linux

```bash
# Debian/Ubuntu
sudo apt install xinput xdotool pulseaudio-utils xclip libnotify-bin

# Arch
sudo pacman -S xorg-xinput pulseaudio xclip libnotify
```

### macOS

```bash
brew install sox
# 需要在系统设置 → 隐私与安全 → 辅助功能中授权终端/altgo
```

### Windows

```powershell
# 1. 安装 Rust 工具链
winget install Rustlang.Rustup

# 2. 安装音频录制工具 (二选一)
winget install ffmpeg  # 推荐
# 或
winget install sox
```

> **提示**：安装 Rustup 时选择默认选项，安装完成后**重新打开 PowerShell** 使 PATH 生效。

## 安装

### 从 Release 下载

前往 [Releases](../../releases) 页面下载对应平台的预编译二进制文件。

### 从源码编译

**所有平台：**

```bash
git clone https://github.com/user/altgo.git
cd altgo
cargo build --release
```

编译产物位于 `target/release/altgo`（Windows 上为 `target/release/altgo.exe`）。

### Linux / macOS 系统安装

```bash
sudo make install
```

默认安装路径：
- 二进制文件：`/usr/local/bin/altgo`
- 配置文件模板：`/etc/altgo/altgo.toml`

### Windows 系统安装

复制 `target/release/altgo.exe` 到任意目录（如 `C:\Program Files\altgo\`），然后将该目录添加到系统 PATH。

## 快速开始

altgo 启动时会校验配置。**首次使用请先完成以下步骤之一**，否则会因缺少 API 密钥而报错退出。

### 方案 A：只要语音转文字，不需要润色（推荐新手）

这种方式**无需任何 API 密钥**，纯本地运行。

**第 1 步：创建配置文件**

**Linux / macOS：**

```bash
mkdir -p ~/.config/altgo
cp configs/altgo.toml ~/.config/altgo/altgo.toml
```

**Windows（PowerShell）：**

```powershell
mkdir -f "$env:APPDATA\altgo"
copy configs\altgo.toml "$env:APPDATA\altgo\altgo.toml"
```

> 配置文件路径：Linux/macOS 为 `~/.config/altgo/altgo.toml`，Windows 为 `%APPDATA%\altgo\altgo.toml`。

**第 2 步：关闭润色**

编辑配置文件，找到 `[polisher]` 段，将 `level` 改为 `"none"`：

```toml
[polisher]
level = "none"   # 关闭润色，无需 API 密钥
```

**第 3 步：运行**

```bash
# Linux / macOS
altgo

# Windows (PowerShell)
.\altgo.exe
```

如果使用本地 whisper.cpp（默认引擎），还需要安装 whisper-cli 并下载模型文件，参见 [语音识别配置](#语音识别配置)。

---

### 方案 B：开启润色功能

需要一个 OpenAI 兼容的 API 密钥（支持 DeepSeek、OpenAI、其他兼容接口）。

**第 1 步：创建配置文件**（同方案 A）

**第 2 步：配置 API 密钥**

在配置文件的 `[polisher]` 段填写密钥和接口：

```toml
[polisher]
engine = "openai"
api_key = "sk-your-key"              # 替换为你的 API 密钥
api_base_url = "https://api.deepseek.com"  # DeepSeek；用 OpenAI 则填 https://api.openai.com
model = "deepseek-chat"              # DeepSeek 模型；OpenAI 填 gpt-3.5-turbo 等
level = "medium"                     # "light" / "medium" / "heavy"
```

或者通过环境变量设置（不写入配置文件）：

```bash
# Linux / macOS
export ALTGO_POLISHER_API_KEY="sk-your-key"
altgo

# Windows (PowerShell)
$env:ALTGO_POLISHER_API_KEY = "sk-your-key"
.\altgo.exe
```

**第 3 步：运行**（同方案 A）

---

### 方案 C：使用云端 Whisper API 转写

如果本地没有安装 whisper.cpp，可以用云端 API 进行语音识别。

编辑配置文件，找到 `[transcriber]` 段：

```toml
[transcriber]
engine = "api"
api_key = "sk-your-key"                    # Whisper API 密钥
api_base_url = "https://api.openai.com"
model = "whisper-1"
language = "zh"
```

或者通过环境变量：

```bash
# Linux / macOS
export ALTGO_TRANSCRIBER_API_KEY="sk-your-key"

# Windows (PowerShell)
$env:ALTGO_TRANSCRIBER_API_KEY = "sk-your-key"
```

---

## 配置详解

配置文件模板位于 `configs/altgo.toml`，所有字段都有默认值，可以只填写需要修改的部分。

```toml
[key_listener]
key_name = "ISO_Level3_Shift"      # 触发键，默认右 Alt
long_press_threshold_ms = 300      # 长按判定阈值 (ms)
double_click_interval_ms = 300     # 双击判定间隔 (ms)

[recorder]
sample_rate = 16000                # 采样率
channels = 1                       # 声道数

[transcriber]
engine = "local"                   # "local"（本地 whisper.cpp）或 "api"（云端 Whisper）
api_key = ""                       # Whisper API 密钥（仅 api 模式需要）
api_base_url = "https://api.openai.com"
model = ""                         # api 模式填模型名如 "whisper-1"；local 模式填模型文件路径如 "~/models/ggml-base.bin"
language = "zh"                    # 语言提示
timeout_seconds = 30

[polisher]
engine = "openai"
api_key = ""                       # LLM API 密钥（level 不为 "none" 时必填）
api_base_url = "https://api.deepseek.com"
model = "deepseek-chat"
level = "medium"                   # "none"（关闭）/ "light"（修标点）/ "medium"（改语序）/ "heavy"（结构化重写）
max_tokens = 1024
timeout_seconds = 60

[output]
enable_notify = true               # 是否显示桌面通知
notify_timeout_ms = 3000

[logging]
level = "info"                     # "debug" / "info" / "warn" / "error"
```

### 语音识别配置

**本地模式**（`engine = "local"`，默认）：

需要安装 [whisper.cpp](https://github.com/ggerganov/whisper.cpp) 并下载 GGML 模型文件：

```bash
# 安装 whisper-cli（以 whisper.cpp 为例）
git clone https://github.com/ggerganov/whisper.cpp.git
cd whisper.cpp && make -j && sudo cp main /usr/local/bin/whisper-cli

# 下载模型（示例：base 模型）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin -O ~/models/ggml-base.bin
```

然后在配置中指定模型路径：

```toml
[transcriber]
engine = "local"
model = "~/models/ggml-base.bin"
```

**API 模式**（`engine = "api"`）：

需要 OpenAI Whisper API 密钥，参见 [方案 C](#方案-c使用云端-whisper-api-转写)。

### 润色级别说明

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

> **优先级**：环境变量 > 配置文件 > 代码默认值。

## 使用

1. 启动 altgo
2. **长按右 Alt** → 开始录音 → 松开停止 → 自动处理 → 文字写入剪贴板
3. **双击右 Alt** → 连续录音 → 再次单击停止 → 自动处理

```bash
# Linux / macOS
altgo                       # 前台运行
altgo --config /path/to/altgo.toml  # 指定配置文件
altgo --version             # 查看版本
altgo --help                # 查看所有选项

# Windows (PowerShell)
.\altgo.exe
.\altgo.exe --config C:\path\to\altgo.toml
.\altgo.exe --version
.\altgo.exe --help
```

## 架构

```
按键事件 → 状态机 → 录音 → ASR 转写 → LLM 润色 → 剪贴板 + 通知
```

```
src/
├── main.rs            # 入口，事件循环
├── config.rs          # TOML 配置加载
├── state_machine.rs   # 按键状态机（长按/双击/连续录音）
├── key_listener/      # 跨平台按键监听
│   ├── mod.rs
│   ├── linux.rs       # xinput test-xi2
│   ├── macos.rs       # CGEvent tap
│   └── windows.rs     # PowerShell hook
├── recorder/          # 跨平台录音
│   ├── mod.rs
│   ├── linux.rs       # parecord
│   ├── macos.rs       # sox/ffmpeg
│   └── windows.rs     # ffmpeg/sox
├── transcriber.rs     # Whisper API + 本地 whisper.cpp
├── polisher.rs        # LLM 文本润色
├── audio.rs           # WAV 编解码，线程安全 Buffer
└── output/            # 跨平台剪贴板 + 通知
    ├── mod.rs
    ├── linux.rs       # xclip/xsel/wl-copy + notify-send
    ├── macos.rs       # pbcopy + osascript
    └── windows.rs     # clip.exe + PowerShell toast
```

## 开发

### Linux / macOS

```bash
# 运行测试
make test

# 代码格式检查
make fmt

# Lint 检查
make lint

# Release 构建
make build
```

### Windows

```powershell
# 运行测试
cargo test

# 代码格式检查
cargo fmt -- --check

# Lint 检查
cargo clippy -- -D warnings

# Release 构建
cargo build --release
```

## 许可证

[MIT](LICENSE)
