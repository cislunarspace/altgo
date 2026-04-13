# CLAUDE.zh-CN.md

本文件为 Claude Code (claude.ai/code) 在此仓库中工作时提供指导（中文版）。

## 项目概述

**altgo** 是一个用 Rust 编写的跨平台桌面语音转文字工具。按住右 Alt 键开始录音，松开后自动进行语音识别（通过 Whisper API 或本地 whisper.cpp）、LLM 文本润色，并将结果复制到剪切板。

## 构建和测试命令

```bash
cargo build --release         # 优化构建
cargo test                    # 运行所有测试
cargo test -- --nocapture     # 运行测试并输出 println
cargo test test_name          # 运行匹配模式的测试
cargo fmt                     # 格式化代码
cargo fmt -- --check          # 检查格式（CI 使用此命令）
cargo clippy -- -D warnings   # 代码检查（警告视为错误）
make build                    # 构建并复制二进制文件到 ./
make install                  # 安装到 /usr/local/bin + /etc/altgo/
```

CI 在三个平台（Linux、macOS、Windows）上运行，检查：`fmt`、`clippy`、`build --release`、`test`。

## 架构

由键盘事件驱动的线性管道：

```
按键监听 → 状态机 → 录音 → 语音识别 → 文本润色 → 输出
```

### 模块说明

- **`main.rs`** — CLI 参数解析（clap），连接所有模块，运行主事件循环。`Transcriber` 枚举在 API 和本地后端之间分派。
- **`config.rs`** — TOML 配置加载，所有字段使用 `serde(default)` 提供默认值。API 密钥可通过 `ALTGO_TRANSCRIBER_API_KEY` 和 `ALTGO_POLISHER_API_KEY` 环境变量覆盖。
- **`state_machine.rs`** — 5 状态枚举（`Idle`、`PotentialPress`、`Recording`、`WaitSecondClick`、`ContinuousRecording`）。长按录音，双击进入连续录音模式。使用 `tokio::select!` 竞争按键事件和超时。
- **`audio.rs`** — 线程安全的 PCM 缓冲区（`Mutex<Vec<u8>>`），WAV 编码/解码（44 字节头 + PCM）。
- **`transcriber.rs`** — `WhisperApi`（HTTP multipart 请求到兼容 OpenAI 的端点）和 `LocalWhisper`（子进程调用 `whisper-cli`）。
- **`polisher.rs`** — LLM 文本润色，支持 4 个级别（`none`/`light`/`medium`/`heavy`）。指数退避重试（最多 3 次）。使用兼容 OpenAI 的聊天 API。
- **`key_listener/`** — 平台特定的按键检测。Linux：`xinput test-xi2`。macOS：通过内联 Swift 使用 CGEvent tap。Windows：PowerShell + `GetAsyncKeyState`。
- **`recorder/`** — 平台特定的音频捕获。Linux：`parecord`。macOS：`sox`。Windows：`ffmpeg`（主选）或 `sox`（备选）。
- **`output/`** — 平台特定的剪切板和通知。Linux：`xclip`/`xsel`/`wl-copy` + `notify-send`。macOS：`pbcopy` + `osascript`。Windows：`clip.exe`/PowerShell + WPF 浮动通知。

### 关键设计模式

**跨平台静态分派** — 每个平台模块（`key_listener`、`recorder`、`output`）在 `mod.rs` 中使用 `#[cfg(target_os = ...)]` 导出统一的类型别名（`PlatformListener`、`PlatformRecorder` 等）。不使用 trait 对象，实现静态分派。

**子进程系统交互** — 所有平台集成都通过调用 CLI 工具实现，而非 FFI。这简化了交叉编译。

**异步通道管道** — `tokio::sync::mpsc` 通道解耦各阶段。按键事件通过无界通道传递，命令通过有界通道（容量 16）。处理作为独立的 `tokio::spawn` 任务运行。

**配置** — 位于 `~/.config/altgo/altgo.toml`。模板在 `configs/altgo.toml`。所有字段都有 serde 默认值，部分配置文件也可以正常工作。

## 平台系统依赖

- **Linux**：`xinput`、`xmodmap`、`parecord`、`xclip`/`xsel`/`wl-copy`、`notify-send`
- **macOS**：`sox`、Swift CLI 工具、`pbcopy`、`osascript`
- **Windows**：`ffmpeg` 或 `sox`、PowerShell

## 测试说明

- 单元测试位于每个源文件的 `#[cfg(test)]` 模块中。
- `config.rs` 和 `audio.rs` 有全面的测试覆盖。
- `transcriber.rs` 和 `polisher.rs` 使用 `mockito` 进行 HTTP 级别的模拟测试。
- 平台特定模块有最少的测试（仅构造/冒烟测试）。
- 尚未创建集成测试目录（`tests/`）。
