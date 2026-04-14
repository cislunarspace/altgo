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

## 配置

首次运行前，复制配置文件模板：

**Linux / macOS：**

```bash
mkdir -p ~/.config/altgo
cp configs/altgo.toml ~/.config/altgo/altgo.toml
```

**Windows（PowerShell）：**

```powershell
# 使用 %APPDATA% 或 %USERPROFILE%
# 例如：C:\Users\YourName\AppData\Roaming\altgo\altgo.toml
# 或：C:\Users\YourName\.config\altgo\altgo.toml
mkdir -p "$env:APPDATA\altgo"
copy configs\altgo.toml "$env:APPDATA\altgo\altgo.toml"
```

编辑配置文件 `altgo.toml`：

```toml
[key_listener]
key_name = "ISO_Level3_Shift"     # 触发键，默认右 Alt
long_press_threshold_ms = 300     # 长按判定阈值 (ms)
double_click_interval_ms = 300      # 双击判定间隔 (ms)

[recorder]
sample_rate = 16000               # 采样率
channels = 1                      # 声道数

[transcriber]
engine = "api"                    # "api" 或 "local"
api_key = ""                      # Whisper API 密钥
api_base_url = "https://api.openai.com"
model = "whisper-1"               # API 模型或本地模型路径
language = "zh"                   # 语言提示

[polisher]
api_key = ""                      # LLM API 密钥
api_base_url = "https://api.deepseek.com"
model = "deepseek-chat"
level = "medium"                  # "none" / "light" / "medium" / "heavy"
max_tokens = 1024

[output]
enable_notify = true              # 是否显示桌面通知
```

### 环境变量

| 变量 | 说明 |
|------|------|
| `ALTGO_TRANSCRIBER_API_KEY` | 覆盖 transcriber.api_key |
| `ALTGO_POLISHER_API_KEY` | 覆盖 polisher.api_key |
| `RUST_LOG` | 日志级别，如 `altgo=debug` |

## 使用

1. 启动 altgo
2. **长按右 Alt** → 开始录音 → 松开停止 → 自动处理 → 文字写入剪贴板
3. **双击右 Alt** → 连续录音 → 再次单击停止 → 自动处理

**Linux / macOS：**

```bash
# 前台运行（查看日志）
altgo

# 指定配置文件
altgo --config /path/to/altgo.toml

# 查看版本
altgo --version
```

**Windows（PowerShell）：**

```powershell
# 前台运行
.\altgo.exe

# 指定配置文件
.\altgo.exe --config C:\path\to\altgo.toml

# 查看版本
.\altgo.exe --version
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
