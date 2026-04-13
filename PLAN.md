# altgo 实施计划

> **主打特点：无需打字，言出法随。快速把用户说的话变成清晰、可用的文字。**

## 概述

altgo 是一个 Linux 桌面后台工具。用户通过右 Alt 键的两种操作模式触发语音录制，松手后自动完成：语音转录 → LLM 纠错润色 → 写入剪贴板 + 桌面通知。整个过程无需切换窗口，不打断工作流。

### 核心交互

| 操作 | 触发 | 行为 |
|------|------|------|
| **长按右 Alt** | 按住说话，松开结束 | 按下 → 录音开始；松开 → 录音结束 → 转录 → 润色 → 输出 |
| **双击右 Alt** | 快速按两次进入，再按一次结束 | 进入连续录音模式；单按结束 → 同上后续流程 |

### 润色级别

| 级别 | 效果 | 适用场景 |
|------|------|----------|
| `none` | 不润色，直接输出 ASR 原文 | 需要原话的场景 |
| `light` | 修正标点和明显错别字 | 日常聊天 |
| `medium` | 修正语法，优化表达使其通顺 | 正式文档、邮件 |
| `heavy` | 重写为有条理的书面文本 | 报告、文章 |

---

## 技术栈

```
语言:        Go 1.22+ (项目名 "altgo" 寓意 Go，goroutine 天然匹配事件驱动架构)
构建工具:    go modules + CGO
键监听:      X11 Record Extension (无需 root)
音频录制:    PulseAudio Simple API (libpulse-simple)
ASR 引擎:    OpenAI Whisper API (优先) / whisper.cpp 本地 (后续)
LLM 润色:    OpenAI-compatible HTTP API (兼容 OpenAI/Claude/Ollama/vLLM)
剪贴板:      xclip / xsel / wl-clipboard (自动检测)
通知:        notify-send
配置:        TOML
日志:        slog (标准库)
```

### 为什么选 Go

1. goroutine + channel 天然适合事件驱动：键监听、录音、ASR、LLM 各阶段解耦并行
2. 编译为单二进制，无运行时依赖，`go install` 即可用
3. CGO 可直接调用 libX11、libpulse 等系统库
4. 项目名 "altgo" 暗示 Go 语言

---

## 系统环境

| 组件 | 状态 | 说明 |
|------|------|------|
| Python 3.12 | 已安装 | 不使用 |
| Go | **未安装** | 需安装 |
| libX11 / libXtst | 已安装 | 键监听 |
| libpulse / libpulse-simple | 已安装 | 音频录制 |
| libevdev | 已安装 | 备选键监听 |
| libnotify / notify-send | 已安装 | 通知 |
| xclip / xsel | **未安装** | 需安装 |
| pipewire | 已安装 | PulseAudio 兼容层 |

---

## 架构设计

### 数据流

```
用户按键 → 状态机判定 → 开始/停止录制
                          ↓
                      音频缓冲区 (PCM → WAV)
                          ↓
                      ASR 转录 → 原始文本
                          ↓
                      LLM 润色 → 成品文字
                          ↓
              ┌───────────┴───────────┐
          写入剪贴板              桌面通知
```

### 并发模型

```
main goroutine
  ├── keyListener goroutine  ──channel: KeyEvent──→  stateMachine goroutine
  │                                                    │ channel: Command
  │                                                    ↓
  │                                                recorder goroutine
  │                                                    │ channel: AudioData
  │                                                    ↓
  └───────────────────────────────────────────── worker (ASR → LLM → Output)
```

各阶段通过 Go channel 通信，解耦且可背压控制。

### 按键状态机

```
Idle (空闲)
  │ Press
  ↓
PotentialPress (可能的长按或双击)
  │ Release(超过长按阈值)          │ Release(未超时)
  ↓                               ↓
Recording (长按录音中)          WaitSecondClick (等待第二次点击)
  │ Release                        │ Press(双击间隔内)     │ Timeout
  ↓                               ↓                       ↓
Processing (处理)              ContinuousRecording     Idle (忽略单按)
                                │ Press
                                ↓
                              Processing (处理)
```

---

## 目录结构

```
altgo/
├── PLAN.md
├── go.mod
├── go.sum
├── Makefile
├── cmd/
│   └── altgo/
│       └── main.go              # 入口
├── internal/
│   ├── config/
│   │   ├── config.go            # 配置结构体 + 加载
│   │   └── config_test.go
│   ├── keylistener/
│   │   ├── listener.go          # 接口定义
│   │   ├── x11.go               # X11 Record 实现
│   │   └── x11_test.go
│   ├── statemachine/
│   │   ├── machine.go           # 按键状态机
│   │   └── machine_test.go
│   ├── recorder/
│   │   ├── recorder.go          # 录音接口
│   │   ├── pulse.go             # PulseAudio 实现
│   │   └── recorder_test.go
│   ├── transcriber/
│   │   ├── transcriber.go       # ASR 接口
│   │   ├── whisper_api.go       # OpenAI Whisper API
│   │   ├── whisper_local.go     # whisper.cpp (TODO)
│   │   └── transcriber_test.go
│   ├── polisher/
│   │   ├── polisher.go          # 润色接口 + 级别枚举
│   │   ├── llm.go               # OpenAI-compatible 实现
│   │   ├── prompts.go           # 各级 prompt 模板
│   │   └── polisher_test.go
│   ├── output/
│   │   ├── clipboard.go         # 剪贴板操作
│   │   ├── notify.go            # 桌面通知
│   │   └── output_test.go
│   └── audio/
│       ├── buffer.go            # 音频缓冲区
│       ├── wav.go               # WAV 编码
│       └── audio_test.go
├── configs/
│   └── altgo.toml               # 默认配置
└── scripts/
    └── install-deps.sh          # 系统依赖安装脚本
```

---

## 实施阶段

### 阶段 0: 环境准备

1. **安装 Go 1.22+**
   ```bash
   wget https://go.dev/dl/go1.22.0.linux-amd64.tar.gz
   sudo tar -C /usr/local -xzf go1.22.0.linux-amd64.tar.gz
   export PATH=$PATH:/usr/local/go/bin:$HOME/go/bin
   ```

2. **安装系统依赖**
   ```bash
   sudo apt install build-essential libx11-dev libxtst-dev \
     libpulse-dev libasound2-dev libevdev-dev libnotify-dev \
     xclip pkg-config
   ```

3. **初始化 Go 项目**
   ```bash
   cd /home/ouyangjiahong/codes/altgo
   go mod init github.com/ouyangjiahong/altgo
   ```

### 阶段 1: 配置系统 + 入口

4. **配置结构体** — `internal/config/config.go`
   - KeyListener: 触发键、长按阈值(ms)、双击间隔(ms)
   - Recorder: 采样率(16000)、声道(1)、位深(16bit)
   - Transcriber: 引擎类型、API key、API base URL、语言
   - Polisher: 引擎、API key、API base URL、模型、润色级别
   - Output: 剪贴板工具、通知开关

5. **默认配置文件** — `configs/altgo.toml`

6. **配置测试** — `internal/config/config_test.go`

7. **程序入口** — `cmd/altgo/main.go`
   - 命令行参数解析
   - 配置加载
   - 日志初始化
   - 信号处理 (SIGINT 优雅退出)

### 阶段 2: 键盘监听 + 状态机

8. **键监听接口** — `internal/keylistener/listener.go`
   ```go
   type KeyEvent struct { KeyCode uint; Pressed bool; Time time.Time }
   type Listener interface { Start(ctx) (<-chan KeyEvent, error); Close() error }
   ```

9. **X11 Record 实现** — `internal/keylistener/x11.go`
   - 双 Display 连接 (ctrl + data)
   - XRecordEnableContextAsync 异步录制
   - 过滤 ISO_Level3_Shift (右 Alt, keycode 108)
   - `runtime.LockOSThread()` 保证线程安全
   - CGO 链接: `-lX11 -lXtst`

10. **按键状态机** — `internal/statemachine/machine.go`
    - 实现 Idle → PotentialPress → Recording / WaitSecondClick → Continuous → Processing 转换
    - 输出 channel 发送 StartRecord / StopRecord 命令

11. **状态机测试** — `internal/statemachine/machine_test.go`
    - 长按场景、双击场景、单按忽略、边界值

### 阶段 3: 音频录制

12. **音频工具** — `internal/audio/buffer.go` + `wav.go`
    - 线程安全缓冲区
    - PCM → WAV 编码 (16kHz, 16bit, mono)

13. **录音接口** — `internal/recorder/recorder.go`
    ```go
    type Recorder interface { Start(ctx) error; Stop() ([]byte, error); Close() error }
    ```

14. **PulseAudio 实现** — `internal/recorder/pulse.go`
    - CGO 调用 pa_simple_new / pa_simple_read
    - goroutine 中循环读取，Stop 时返回完整 WAV
    - CGO 链接: `-lpulse-simple -lpulse`

### 阶段 4: ASR 转录

15. **ASR 接口** — `internal/transcriber/transcriber.go`
    ```go
    type Result struct { Text string; Language string; Duration time.Duration }
    type Transcriber interface { Transcribe(ctx, audioData) (*Result, error) }
    ```

16. **OpenAI Whisper API** — `internal/transcriber/whisper_api.go`
    - POST /v1/audio/transcriptions (multipart)
    - 支持自定义 API base URL
    - 超时 30s，错误重试

17. **ASR 测试** — `internal/transcriber/transcriber_test.go`
    - httptest.Server mock

### 阶段 5: LLM 润色

18. **润色接口 + Prompt** — `internal/polisher/polisher.go` + `prompts.go`
    - PolishLevel: none / light / medium / heavy
    - 各级别 system prompt 定义

19. **LLM 实现** — `internal/polisher/llm.go`
    - POST /v1/chat/completions
    - 支持自定义 API base (兼容 Ollama/vLLM)
    - 超时 60s，指数退避重试 3 次
    - PolishNone 时跳过调用

20. **润色测试** — `internal/polisher/polisher_test.go`

### 阶段 6: 输出

21. **剪贴板** — `internal/output/clipboard.go`
    - 自动检测 xclip > xsel > wl-copy
    - 检测 X11 vs Wayland

22. **桌面通知** — `internal/output/notify.go`
    - notify-send 方案
    - 显示 "正在处理..." → "已完成" 两阶段通知

### 阶段 7: 集成串联

23. **主流程编排** — `cmd/altgo/main.go` (更新)
    - 串联: listener → stateMachine → recorder → transcriber → polisher → output
    - 处理 goroutine 生命周期和错误传播

24. **Makefile** — 构建、测试、安装目标
25. **README.md** — 安装、配置、使用说明

---

## 风险与缓解

| 风险 | 严重度 | 缓解措施 |
|------|--------|----------|
| Wayland 下 X11 Record 不工作 | 高 | 检测 XDG_SESSION_TYPE；备选 evdev；远期 DBus GlobalShortcuts |
| CGO 调试复杂 | 中 | CGO 封装隔离，接口层纯 Go；LockOSThread |
| ASR/LLM 延迟影响体验 | 中 | "处理中"通知；合理超时；后续支持本地 whisper |
| xclip 未安装 | 中 | install-deps.sh 包含；启动时检测并提示 |
| Go 未安装 | 低 | 阶段 0 明确安装步骤 |

---

## 后续优化

1. 本地 whisper.cpp 集成 (离线、低延迟)
2. evdev 键监听 (Wayland native)
3. VAD 语音活动检测 (自动检测说话结束)
4. 系统托盘图标 (显示状态)
5. 模拟键盘输入 (xdotool，替代剪贴板)
6. 自定义快捷键
7. DEB/RPM 打包

---

## 成功标准

- [ ] 长按右 Alt → 说话 → 松开 → 剪贴板出现润色后的文字
- [ ] 双击右 Alt → 连续录音 → 单按停止 → 同上
- [ ] 润色级别可通过配置切换
- [ ] 桌面通知显示处理状态和结果
- [ ] SIGINT 优雅退出
- [ ] 单元测试覆盖率 >= 80%
