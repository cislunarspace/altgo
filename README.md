# altgo

**无需打字，言出法随** — 跨平台语音转文字桌面工具

按住右 Alt 键说话，松开后在本地用 **whisper.cpp** 转写，可选通过 **任意兼容 OpenAI Chat API 的 LLM** 润色；结果在**悬浮窗**中展示，由你自行选择是否复制，**不会自动写入系统剪贴板**。

本仓库**主要面向 Linux 用户**：在 Windows 上已有大量同类工具，而能在 Linux 上持续开发、维护的同类项目仍然很少。Windows 版仅作附带支持，安装与排障说明相对简略。

## 功能

- **长按触发**：长按右 Alt 键进入录音模式，松开自动停止并处理
- **双击切换**：双击右 Alt 键进入连续录音模式，再次单击停止
- **本地 ASR**：以 **whisper.cpp** 为主；`whisper-cli` 与 **ffmpeg** 等随 **官方预编译包** 一并提供，一般无需自行安装；模型可在**设置**里下载并选用
- **LLM 润色（可选）**：通过 **OpenAI 兼容的 HTTP API** 调用任意厂商或本地部署的 LLM（如云端 API、Ollama、vLLM 等），对转写文本做 light / medium / heavy 等档位润色
- **悬浮窗结果**：处理完成后弹出悬浮窗展示文本，由用户选择复制；不自动覆盖剪贴板
- **桌面通知**：处理完成时可伴随通知提示（依配置）

## 系统要求（Linux）

- **测试与部署环境**：目前仅在 **Ubuntu 20.04** 上做过安装与运行验证；其他发行版可能可用，但未保证。
- **读取键盘设备（必做）**：在常见安装方式下，必须将当前用户加入 **`input` 组**，否则无法稳定访问 `/dev/input/event*`，按键监听会失败。执行后**须重新登录**会话方可生效：

  ```bash
  sudo usermod -aG input "$USER"
  # 注销并重新登录，或重启后再试
  ```

- **其余系统组件**（如与桌面、音频、通知相关的库）由 **`.deb` 的依赖关系** 或发行说明处理：缺什么按安装器提示补装即可，不必手工对照长清单。

## 系统托盘

启动后会在系统托盘显示图标，点击图标可显示/隐藏主窗口，右键菜单提供「显示窗口」和「退出」选项。

## 安装

### 给最终用户（推荐）

**Linux（主要平台）**

1. 前往 [Releases](../../releases) 下载 **`.deb`** 或 **AppImage**。
2. 安装 **`.deb`**（若提示依赖不足，执行 `sudo apt -f install` 或按提示补装）。**AppImage**：`chmod +x` 后直接运行，也可配合 [AppImageLauncher](https://github.com/TheAssassin/AppImageLauncher)。
3. **务必**完成 [系统要求](#系统要求linux) 中的 **`input` 组** 步骤（与按键监听相关，安装包无法代劳）。
4. 启动应用，在 **[设置](#首次使用应用内设置)** 里完成转写模型与可选润色等；**不要**一上来编辑配置文件。

**预编译包与捆绑内容**：官方构建会把 **ffmpeg**、**whisper-cli** 等与程序一起打进 **deb / AppImage / MSI / zip**，目标是 **安装或解压后开箱即用**，无需再为录音与转写去单独安装这些二进制。

**Windows（附带支持）**

1. 从 [Releases](../../releases) 安装 **MSI** 或使用 zip 绿色版。
2. 同上，**ffmpeg 等已随包提供**；直接打开应用，在 **设置** 中完成首次配置即可。

### 给开发者（从本仓库构建）

克隆仓库后，使用 **`make build`** 可一次性拉取 **whisper-cli、ffmpeg** 等到 `target/deps/bin/`，执行 `cargo tauri build`，并把依赖二进制拷贝到 `src-tauri/target/release/bin/`，与 CI/打包流程一致。**日常联调与验证同样建议以 `make build` 为主**，确保与发布产物行为一致。

```bash
git clone <本仓库 URL>
cd altgo
cd frontend && npm install && cd ..
# Linux：先安装 Tauri 所需的 GTK/WebKit 等开发包，见下文「开发环境」
make deps-linux    # 或 Windows：make deps-windows / pwsh packaging/scripts/download-deps.ps1
make build
# 可选：sudo make install
```

若需快速改前端界面，可临时使用 `cargo tauri dev` 获得热重载；**完整链路（含捆绑二进制、与发布一致）仍以 `make build` 为准。**

从源码自行构建且**未**走 `make deps-*` 时，才需要自行保证 **ffmpeg** / **whisper-cli** 可被程序找到（例如加入 `PATH` 或执行 `make deps-windows`）。

## 首次使用：应用内设置

安装并启动后，**面向用户的选项都应在图形界面里完成**，无需先理解配置文件：

- **顶部状态**：会提示本地转写是否就绪（例如是否已选用可用模型）；按提示操作即可。
- **转写**：选择 **本地 whisper.cpp** 或 **云端 API**（若使用）；设置识别语言；在 **模型管理** 中 **下载 / 选用** 模型，或使用「高级」填写本机 `.bin` 路径或模型名。
- **润色**：选择是否启用以及轻/中/重度；填写兼容 **OpenAI Chat API** 的地址、模型名与密钥（适用于云端或本地网关如 Ollama 等）。
- **外观**：浅色 / 深色 / 跟随系统；**界面语言**。
- **录音 / 触发键**：预设左右 Alt 或 **「按下以设置」** 捕获快捷键。
- 点击 **保存**；多数情况下管道会自动重载，无需重启应用。

跟着界面走即可完成日常使用。

## 高级：直接编辑配置文件（可选）

仅在需要 **脚本化、批量部署、或与 GUI 未暴露的字段打交道** 时使用：

- **Linux**：`~/.config/altgo/altgo.toml`
- **Windows**：`%APPDATA%\altgo\altgo.toml`

仓库内 [`configs/altgo.toml`](configs/altgo.toml) 列出全部字段及注释；与界面保存的是同一套配置。

### 环境变量（高级 / 部署）

| 变量 | 说明 |
|------|------|
| `ALTGO_POLISHER_API_KEY` | 覆盖润色 API 密钥 |
| `ALTGO_TRANSCRIBER_API_KEY` | 若使用云端转写 API，可覆盖其密钥 |
| `RUST_LOG` | 日志级别，如 `altgo=debug` |

## 使用

1. 启动 altgo。
2. **长按右 Alt** → 录音 → 松开 → 本地转写（及可选润色）→ **悬浮窗**展示结果，自行选择是否复制。
3. **双击右 Alt** → 连续录音 → 再次单击停止 → 同上。

### 按 Alt 没有反应？

1. **默认触发键是右侧 Alt**。优先在 **设置 → 录音 / 触发键** 里用「按下以设置」或预设；一般不必改配置文件。
2. **Linux：是否已加入 `input` 组并重新登录？** 未满足则 Wayland/X11 下按键设备常无法读取。
3. 查看主窗口是否报错：模型缺失、`xinput`/`evtest` 不可用等会导致管道无法就绪。
4. 调试：`RUST_LOG=altgo=debug altgo`。

## 架构

```
按键事件 → 状态机 → 录音 → whisper.cpp 转写 → 可选 LLM 润色 → 悬浮窗展示 + 通知
```

altgo 基于 **Tauri**，前端 **React**，核心逻辑 **Rust**。

## 开发环境

### 前置依赖

- Rust stable（建议 **1.80+**，见 [Tauri 2 前置条件](https://tauri.app/start/prerequisites/)）
- **Node.js 18+**（建议 20+）
- Tauri CLI：`cargo install tauri-cli --version "^2"`

### Ubuntu 20.04 上打包依赖示例

```bash
sudo apt update
sudo apt install build-essential curl wget file \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
```

仅需可执行文件、不生成 deb 时：`cargo tauri build --no-bundle`。

### 常用命令

```bash
cd frontend && npm install
make deps-linux && make build     # 推荐：与发布一致

cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path=src-tauri/Cargo.toml
cd frontend && npm run build
```

### Makefile 摘要

| 目标 | 说明 |
|------|------|
| `make deps-linux` / `make deps-windows` | 下载 whisper-cli、ffmpeg 等至 `target/deps/bin/` |
| `make build` | 依赖上述二进制后 `cargo tauri build`，并拷贝到 `src-tauri/target/release/bin/` |
| `make install` | 安装可执行文件与 `/etc/altgo` 配置（通常需 `sudo`） |

## 相关文档

- [CONTRIBUTING.md](CONTRIBUTING.md)（含 **CI / Release / GitHub Pages** 维护说明）
- [CLAUDE.md](CLAUDE.md)
- [docs-site/](docs-site/)

## 许可证

[MIT](LICENSE)
