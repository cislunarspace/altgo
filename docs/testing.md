# 测试套件现状

本文档梳理 altgo 的测试套件：规模、分布、运行真实性、覆盖缺口。
数据来自 2026-08-10 对代码库的逐模块盘点与 Linux 真实运行基线。
面向维护者：了解现状与保持测试质量的提示从这里开始。

## 规模

共 234 个 Rust lib 测试（`cargo test --lib`），前端 vitest 30 个。无 Rust 集成测试目录，无端到端测试。

Rust 测试按模块分布：

| 模块 | 测试数 | 性质 |
|---|---|---|
| voice_pipeline | 27 | 端到端与 handler 行为 |
| config | 24 | 全行为，质量好 |
| recorder | 23 | 全行为，含 DSP |
| overlay | 20 | manager 与 tauri 适配 |
| audio | 17 | 全行为 |
| model | 14 | 下载、列表、删除 |
| polisher | 12 | OpenAI 与 Anthropic 协议、重试 |
| state_machine | 11 | 全行为 |
| sherpa | 3 | 模型/词表缺失报错、语言归一化 |
| tauri_sink | 10 | 脱 Wry 后真实运行 |
| transcriber | 0 | trait-only，经 sherpa.rs 覆盖 |
| history | 9 | 全行为 |
| config_store | 9 | 补丁、校验、持久化 |
| cmd | 9 | copy_text、download_model 事件等 |
| key_capture | 7 | 6 行为 1 冒烟 |
| prompt_store | 6 | 模板加载 |
| key_listener | 6 | Linux 为主 |
| error | 5 | 错误分类 |
| pipeline_controller | 4 | 状态机生命周期 |
| resource | 1 | 路径解析 |
| output | 1 | 剪贴板写入 |

质量分层：绝大多数是有断言的行为测试，少量为构造/冒烟测试。测试文化整体偏行为，非凑数。

前端测试：overlay.test.ts（16）、useConfigForm.test.ts（6）、
overlay.transitions.test.tsx（4）、StatusIndicator.test.tsx（4）。
vitest + jsdom + Testing Library。

## 运行真实性

按 CI 实际执行分四类：

| 类别 | 内容 | 数量 |
|---|---|---|
| 真实运行（Linux `check` job） | 平台无关 + 仅 Linux 测试 | 234 |
| Linux `amd64` / `arm64` CI job | 平台无关 + Linux 测试 | 约 194 |

关键事实：

- Linux 基线：`cargo test --lib` 234 个全绿，无跳过、无失败、无 `#[ignore]`。
- 前端基线：vitest 30 个全绿。
- `tauri_sink.rs` 的 10 个测试已完成脱 Wry 改造（issue #104），现在在 Linux 真实运行。
- CI 的 `amd64` 与 `arm64` 两个 job 均跑 `cargo test --lib`。
- `release.yml` 有独立的双架构 `test` job；deb 与 rpm 构建均 `needs: [test]`，发版前必须通过两种架构的测试。
- fmt/clippy 已加 `--all-targets`，测试代码也会接受 clippy 编译检查。

## 覆盖缺口

此前记录的大、中、小缺口已全部补齐，包括：

- 重大缺口：model 下载（成功、HTTP 失败、小于 10MB 损坏检测、3 次重试、双 base 回退、URL 覆盖）、Anthropic 协议头与响应、polisher 重试耗尽与瞬时失败恢复、cmd.rs 核心命令。
- 中缺口：handle_stop_record 成功链路、dispatch_history_polish、PipelineContext 端到端、config_store 非原子污染。
- 小缺口：WAV 解码异常分支、DSP 升采样/NaN/极小输入、state_machine 连发边界、overlay 失败路径、config 权限位。

新增功能应继续遵循既有测试替身范式：FakeRecorder、FakeTranscriber、MockSink、FakeListener、RecordingOverlayWindow、mockito HTTP mock。保持可注入，就能保持可测。

## 补测试路线图

本路线图已全部完成（2026-08-10）。后续新增功能按上节提示随代码一起补测试。
