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

## 快速安装

提供一键安装脚本，自动下载依赖、编译、生成配置。

### Linux

```bash
git clone https://github.com/cislunarspace/altgo.git
cd altgo
chmod +x install.sh
./install.sh            # 默认下载 base 模型
./install.sh --model small   # 或指定模型大小
```

脚本会自动：检查 Rust 工具链 → 编译 altgo → 编译 whisper.cpp → 下载模型 → 生成配置文件。

如缺少系统依赖（xinput、parecord 等），脚本会提示安装命令，不会自动执行 `sudo`。

### Windows

```powershell
git clone https://github.com/cislunarspace/altgo.git
cd altgo
.\install.ps1               # 默认下载 base 模型
.\install.ps1 -Model small  # 或指定模型大小
```

脚本会自动：检查 Rust 工具链 → 编译 altgo → 下载 whisper.cpp 预编译二进制 → 下载模型 → 生成配置文件。

### 安装选项

| Linux 参数 | Windows 参数 | 说明 |
|-----------|-------------|------|
| `--skip-rust` | `-SkipRust` | 跳过 Rust 工具链检查 |
| `--skip-build` | `-SkipBuild` | 跳过编译 altgo |
| `--skip-whisper` | `-SkipWhisper` | 跳过 whisper.cpp 安装 |
| `--skip-model` | `-SkipModel` | 跳过模型下载 |
| `--model <name>` | `-Model <name>` | 模型大小：tiny, base, small, medium, large |

---

## 手动安装

### 从 Release 下载

前往 [Releases](../../releases) 页面下载对应平台的预编译二进制文件。

### 从源码编译

```bash
git clone https://github.com/cislunarspace/altgo.git
cd altgo
cargo build --release
```

编译产物位于 `target/release/altgo`（Windows 上为 `target/release/altgo.exe`）。

---

## Linux 配置指南

### 1. 安装系统依赖

```bash
# Debian/Ubuntu
sudo apt install xinput xdotool pulseaudio-utils xclip libnotify-bin

# Arch
sudo pacman -S xorg-xinput pulseaudio xclip libnotify
```

altgo 使用 `xinput test-xi2` 监听按键、`parecord` 录音、`xclip`/`xsel`/`wl-copy` 操作剪贴板、`notify-send` 发送通知。以上任一工具缺失都会导致对应功能无法使用。

### 2. 安装 altgo

```bash
# 方式一：下载 Release 二进制文件，放入 PATH
sudo cp altgo /usr/local/bin/

# 方式二：从源码编译后系统安装（同时安装配置文件模板）
sudo make install
```

`make install` 会将二进制文件安装到 `/usr/local/bin/altgo`，配置文件模板安装到 `/etc/altgo/altgo.toml`。

### 3. 配置语音识别（二选一）

#### 方式 A：本地 whisper.cpp（推荐，无需 API 密钥）

```bash
# 编译 whisper.cpp
git clone https://github.com/ggerganov/whisper.cpp.git
cd whisper.cpp
make -j

# 将 main 可执行文件复制到 PATH 中，并命名为 whisper-cli
sudo cp main /usr/local/bin/whisper-cli
```

> **提示**：altgo 通过配置文件中的 `whisper_path` 字段指定 whisper-cli 路径，也支持在 PATH 中自动查找 `whisper-cli` 或 `whisper-cpp`。

下载 GGML 模型文件：

```bash
mkdir -p ~/models
# base 模型（约 148 MB）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin -O ~/models/ggml-base.bin

# 如需中文效果更好，推荐 medium 或 large 模型
# wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin -O ~/models/ggml-medium.bin
```

验证安装：

```bash
whisper-cli -m ~/models/ggml-base.bin -l zh -f test.wav --no-timestamps
```

如果输出转写文字，说明安装成功。

#### 方式 B：云端 Whisper API

无需本地安装 whisper.cpp，使用云端 API 进行语音识别。参见下方 [使用云端 Whisper API 转写](#使用云端-whisper-api-转写)。

### 4. 创建配置文件

```bash
mkdir -p ~/.config/altgo
cp configs/altgo.toml ~/.config/altgo/altgo.toml
```

编辑 `~/.config/altgo/altgo.toml`，至少配置以下内容：

**只要语音转文字，不需要润色：**

```toml
[transcriber]
engine = "local"
model = "~/models/ggml-base.bin"   # 模型文件路径
whisper_path = ""                   # 留空自动在 PATH 中查找，或填写 whisper-cli 绝对路径
language = "zh"

[polisher]
level = "none"   # 关闭润色，无需 API 密钥
```

**如需开启润色功能：**参见下方 [润色配置](#润色配置)。

### 5. 运行

```bash
altgo                        # 前台运行
altgo --config /path/to/altgo.toml  # 指定配置文件
altgo --version              # 查看版本
altgo --help                 # 查看所有选项
```

---

## Windows 配置指南

### 1. 安装系统依赖

```powershell
# 1. 安装 Rust 工具链（从源码编译时需要）
winget install Rustlang.Rustup
# 安装时选择默认选项，安装完成后**重新打开 PowerShell** 使 PATH 生效

# 2. 安装音频录制工具（二选一，ffmpeg 推荐）
winget install ffmpeg     # 推荐，altgo 优先使用 ffmpeg dshow 接口录音
# 或
winget install sox        # 备选，仅当 ffmpeg 不可用时使用
```

> **ffmpeg vs sox**：altgo 在 Windows 上优先使用 ffmpeg（通过 dshow 接口）。它会自动在 PATH 和 winget 安装目录中搜索 ffmpeg。如果 ffmpeg 不可用，会回退到 sox。两者都未安装则录音失败。

### 2. 安装 altgo

从 Release 下载 `altgo.exe`，复制到任意目录（如 `C:\Program Files\altgo\`），然后将该目录添加到系统 PATH。

或从源码编译：

```powershell
cargo build --release
# 产物位于 target\release\altgo.exe
```

### 3. 配置语音识别（二选一）

#### 方式 A：本地 whisper.cpp（推荐，无需 API 密钥）

**步骤 3.1：获取 whisper-cli.exe**

whisper.cpp 官方提供预编译 Windows Release。前往 [whisper.cpp Releases](https://github.com/ggerganov/whisper.cpp/releases) 页面，下载最新的 `whisper-bin-x64.zip`。

解压后，将 `main.exe`（或 `whisper-cli.exe`）及其附带的 DLL 文件复制到一个固定目录：

```powershell
mkdir "C:\Program Files\whisper"
# 将解压目录中的 exe 和 dll 全部复制过去
copy bin\* "C:\Program Files\whisper\"
```

> **提示**：altgo 支持通过配置文件中的 `whisper_path` 字段直接指定 whisper-cli 路径，无需添加到 PATH。也可将目录加入 PATH，altgo 会自动查找 `whisper-cli` 或 `whisper-cpp`。

如果需要加入 PATH：

```powershell
# 以管理员身份运行 PowerShell
[Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\Program Files\whisper", "Machine")
```

或者，如果你从源码编译 whisper.cpp：

```powershell
# 需要 CMake 和 Visual Studio Build Tools
git clone https://github.com/ggerganov/whisper.cpp.git
cd whisper.cpp
cmake -B build
cmake --build build --config Release
# 产物位于 build\bin\Release\ 目录下
copy build\bin\Release\* "C:\Program Files\whisper\"
```

**步骤 3.2：下载模型文件**

```powershell
mkdir "$env:USERPROFILE\models"
# base 模型（约 148 MB）
Invoke-WebRequest -Uri "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin" -OutFile "$env:USERPROFILE\models\ggml-base.bin"

# 如需中文效果更好，推荐 medium 或 large 模型
# Invoke-WebRequest -Uri "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin" -OutFile "$env:USERPROFILE\models\ggml-medium.bin"
```

**步骤 3.3：验证安装**

```powershell
whisper-cli -m "$env:USERPROFILE\models\ggml-base.bin" -l zh -f test.wav --no-timestamps
```

如果输出转写文字，说明安装成功。如果提示"命令未找到"，检查 `whisper-cli.exe` 是否在 PATH 中（**重启 PowerShell** 后再试）。

#### 方式 B：云端 Whisper API

无需本地安装 whisper.cpp，使用云端 API 进行语音识别。参见下方 [使用云端 Whisper API 转写](#使用云端-whisper-api-转写)。

### 4. 创建配置文件

```powershell
mkdir "$env:APPDATA\altgo"
copy configs\altgo.toml "$env:APPDATA\altgo\altgo.toml"
```

> **配置文件路径**：Windows 上为 `%APPDATA%\altgo\altgo.toml`（通常是 `C:\Users\<用户名>\AppData\Roaming\altgo\altgo.toml`）。

编辑配置文件，至少配置以下内容：

**只要语音转文字，不需要润色：**

```toml
[transcriber]
engine = "local"
# Windows 路径可以用正斜杠或反斜杠，正斜杠更安全（无需转义）
model = "C:/Users/<你的用户名>/models/ggml-base.bin"
whisper_path = "C:/Program Files/whisper/whisper-cli.exe"   # 或留空自动查找
language = "zh"

[polisher]
level = "none"
```

> **路径提示**：`model` 和 `whisper_path` 字段支持绝对路径。`whisper_path` 留空时，altgo 会在 PATH 中自动查找 `whisper-cli` 或 `whisper-cpp`。推荐使用正斜杠 `/` 避免转义问题。

**如需开启润色功能：**参见下方 [润色配置](#润色配置)。

### 5. 运行

```powershell
.\altgo.exe
.\altgo.exe --config C:\path\to\altgo.toml
.\altgo.exe --version
.\altgo.exe --help
```

> **提示**：运行时 PowerShell 窗口需要保持打开。如需后台运行，可使用 `Start-Process -WindowStyle Hidden .\altgo.exe`。

---

## macOS 配置指南

### 1. 安装系统依赖

```bash
brew install sox
```

altgo 在 macOS 上优先使用 `sox` 进行录音。如果 sox 不可用，会回退到 `ffmpeg`（需单独安装）。

> **辅助功能权限**：altgo 使用 CGEvent tap 监听按键，需要在 **系统设置 → 隐私与安全性 → 辅助功能** 中授权终端或 altgo 应用。首次运行时如果没有授权，按键监听将无法工作。

### 2. 安装 altgo

```bash
# 方式一：下载 Release 二进制文件，放入 PATH
sudo cp altgo /usr/local/bin/

# 方式二：从源码编译后系统安装
sudo make install
```

`make install` 会将二进制文件安装到 `/usr/local/bin/altgo`，配置文件模板安装到 `/etc/altgo/altgo.toml`。

### 3. 配置语音识别（二选一）

#### 方式 A：本地 whisper.cpp（推荐，无需 API 密钥）

```bash
# 编译 whisper.cpp
git clone https://github.com/ggerganov/whisper.cpp.git
cd whisper.cpp
make -j

# 将 main 可执行文件复制到 PATH 中，并命名为 whisper-cli
sudo cp main /usr/local/bin/whisper-cli
```

> **提示**：altgo 通过配置文件中的 `whisper_path` 字段指定 whisper-cli 路径，也支持在 PATH 中自动查找 `whisper-cli` 或 `whisper-cpp`。

下载 GGML 模型文件：

```bash
mkdir -p ~/models
# base 模型（约 148 MB）
curl -L -o ~/models/ggml-base.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

# 如需中文效果更好，推荐 medium 或 large 模型
# curl -L -o ~/models/ggml-medium.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin
```

验证安装：

```bash
whisper-cli -m ~/models/ggml-base.bin -l zh -f test.wav --no-timestamps
```

如果输出转写文字，说明安装成功。

#### 方式 B：云端 Whisper API

无需本地安装 whisper.cpp，使用云端 API 进行语音识别。参见下方 [使用云端 Whisper API 转写](#使用云端-whisper-api-转写)。

### 4. 创建配置文件

```bash
mkdir -p ~/.config/altgo
cp configs/altgo.toml ~/.config/altgo/altgo.toml
```

编辑 `~/.config/altgo/altgo.toml`，至少配置以下内容：

**只要语音转文字，不需要润色：**

```toml
[transcriber]
engine = "local"
model = "~/models/ggml-base.bin"
whisper_path = ""                   # 留空自动在 PATH 中查找，或填写 whisper-cli 绝对路径
language = "zh"

[polisher]
level = "none"
```

**如需开启润色功能：**参见下方 [润色配置](#润色配置)。

### 5. 运行

```bash
altgo                        # 前台运行
altgo --config /path/to/altgo.toml  # 指定配置文件
altgo --version              # 查看版本
altgo --help                 # 查看所有选项
```

---

## 使用

1. 启动 altgo
2. **长按右 Alt** → 开始录音 → 松开停止 → 自动处理 → 文字写入剪贴板
3. **双击右 Alt** → 连续录音 → 再次单击停止 → 自动处理

---

## 使用云端 Whisper API 转写

如果你不想安装本地 whisper.cpp，可以使用云端 Whisper API。编辑配置文件中的 `[transcriber]` 段：

```toml
[transcriber]
engine = "api"
api_key = "sk-your-key"                    # Whisper API 密钥
api_base_url = "https://api.openai.com"    # API 地址
model = "whisper-1"                        # 模型名称
language = "zh"
```

或通过环境变量设置密钥：

```bash
# Linux / macOS
export ALTGO_TRANSCRIBER_API_KEY="sk-your-key"

# Windows (PowerShell)
$env:ALTGO_TRANSCRIBER_API_KEY = "sk-your-key"
```

---

## 润色配置

altgo 支持通过 LLM 对转写文本进行润色。需要配置 API 密钥和 Provider 信息。

### 润色级别

| 级别 | 效果 | 需要 API 密钥 |
|------|------|--------------|
| `none` | 不润色，直接输出转写文本 | 否 |
| `light` | 修正标点和错别字 | 是 |
| `medium` | 修正语法、改善语序通顺度 | 是 |
| `heavy` | 结构化重写，适合正式文档 | 是 |

### 配置示例

编辑配置文件中的 `[polisher]` 段：

```toml
[polisher]
protocol = "openai"            # "openai"（OpenAI/DeepSeek 等）或 "anthropic"
api_key = "sk-your-key"        # 你的 API 密钥
api_base_url = "https://..."   # 你的 Provider API 地址
model = "your-model"           # 使用的模型名称
level = "medium"               # "light" / "medium" / "heavy"
```

或通过环境变量设置密钥（其他字段仍需写在配置文件中）：

```bash
# Linux / macOS
export ALTGO_POLISHER_API_KEY="sk-your-key"

# Windows (PowerShell)
$env:ALTGO_POLISHER_API_KEY = "sk-your-key"
```

### Provider 配置参考

**OpenAI：**

```toml
[polisher]
protocol = "openai"
api_key = "sk-your-key"
api_base_url = "https://api.openai.com"
model = "gpt-3.5-turbo"
```

**DeepSeek：**

```toml
[polisher]
protocol = "openai"
api_key = "sk-your-key"
api_base_url = "https://api.deepseek.com"
model = "deepseek-chat"
```

**Anthropic：**

```toml
[polisher]
protocol = "anthropic"
api_key = "sk-ant-your-key"
api_base_url = "https://api.anthropic.com"
model = "claude-sonnet-4-20250514"
```

**本地 Ollama（OpenAI 兼容）：**

```toml
[polisher]
protocol = "openai"
api_key = "ollama"
api_base_url = "http://localhost:11434"
model = "qwen2.5:7b"
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
model = ""                         # api 模式填模型名如 "whisper-1"；local 模式填模型文件路径
whisper_path = ""                  # whisper-cli 二进制路径，留空自动在 PATH 中查找
language = "zh"                    # 语言提示
timeout_seconds = 30

[polisher]
protocol = "openai"                # "openai"（OpenAI/DeepSeek 等）或 "anthropic"
api_key = ""                       # LLM API 密钥（level 不为 "none" 时必填）
api_base_url = ""                  # Provider API 地址（level 不为 "none" 时必填）
model = ""                         # 模型名称（level 不为 "none" 时必填）
level = "none"                     # "none" / "light" / "medium" / "heavy"
max_tokens = 1024
timeout_seconds = 60

[output]
enable_notify = true               # 是否显示桌面通知
notify_timeout_ms = 3000

[logging]
level = "info"                     # "debug" / "info" / "warn" / "error"
```

### 环境变量

| 变量 | 说明 |
|------|------|
| `ALTGO_TRANSCRIBER_API_KEY` | 覆盖 `transcriber.api_key` |
| `ALTGO_POLISHER_API_KEY` | 覆盖 `polisher.api_key` |
| `RUST_LOG` | 日志级别，如 `altgo=debug` |

> **优先级**：环境变量 > 配置文件 > 代码默认值。

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
make test      # 运行测试
make fmt       # 代码格式检查
make lint      # Lint 检查
make build     # Release 构建
```

### Windows

```powershell
cargo test                    # 运行测试
cargo fmt -- --check          # 代码格式检查
cargo clippy -- -D warnings   # Lint 检查
cargo build --release         # Release 构建
```

## 许可证

[MIT](LICENSE)
