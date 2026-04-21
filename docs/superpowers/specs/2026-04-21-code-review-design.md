# 全面代码审查设计方案

## 目标

对 altgo 项目进行深度代码审查，发现潜在的 bug、并发安全问题、错误处理问题、资源管理问题、安全漏洞等。

## 审查范围

### Rust 后端 (`src-tauri/src/`)

| 模块 | 审查重点 |
|------|----------|
| `lib.rs` | Tauri 入口、状态管理、AppState 结构 |
| `cmd.rs` | 命令处理、管道运行、配置保存 |
| `state_machine.rs` | 状态机正确性、竞态条件 |
| `pipeline.rs` | 管道处理、async 流程 |
| `key_listener/` | Linux/X11 按键监听、evtest fallback |
| `key_capture.rs` | 激活键捕获、平台特定代码 |
| `recorder/` | 录音模块、进程管理 |
| `transcriber.rs` | whisper.cpp / API 转写 |
| `polisher.rs` | LLM 润色、API 调用 |
| `history.rs` | 历史记录 JSON 管理 |
| `config.rs` | 配置加载验证 |
| `output/` | 剪贴板/通知平台实现 |
| `audio.rs` | WAV 编解码、 PCM buffer |
| `model.rs` | 模型下载管理 |

### 前端 (`frontend/src/`)

| 模块 | 审查重点 |
|------|----------|
| `App.tsx` | 路由配置 |
| `Settings.tsx` | 设置页状态管理、API 调用 |
| `History.tsx` | 历史页 CRUD |
| `ThemeContext.tsx` | 主题状态管理 |
| `components/` | UI 组件正确性 |
| `hooks/useTauri.ts` | Tauri 集成 |
| `i18n/` | 国际化配置 |

## 审查维度

### 1. 并发安全
- Mutex/RwLock 使用是否正确
- Channel 生命周期管理
- Arc/Clone 共享状态是否安全
- 线程创建和 join 清理

### 2. 错误处理
- Result/Option 是否正确处理
- unwrap/expect 风险点
- panic 恢复机制
- 边界情况和空状态

### 3. 资源管理
- 文件句柄泄漏
- 子进程清理
- 线程清理（尤其是在 Drop 实现中）
- 锁死锁风险

### 4. 状态机
- 状态转换是否完整
- 竞态条件
- 超时处理

### 5. 输入验证
- 配置字段校验
- API 响应处理
- 路径安全

### 6. 安全
- 进程注入风险
- 路径遍历
- 密钥处理
- Shell 命令注入

### 7. 逻辑错误
- 业务逻辑 bug
- 边界条件
- 假设与实际不符

### 8. 性能
- 内存分配优化
- 循环效率
- 不必要的 clone

## 工作方式

1. **并行分析**：Rust 和前端同时检查
2. **问题记录**：每个问题记录：文件、位置、类型、严重程度、修复建议
3. **严重程度分级**：
   - 🔴 严重：会导致崩溃、数据丢失、安全问题
   - 🟠 中等：功能异常、性能问题
   - 🟡 低：代码质量、潜在风险

## 输出

- 问题清单（按严重程度排序）
- 每个问题包含：文件路径、行号、问题描述、修复建议

## 执行步骤

1. 深度分析 Rust 后端核心模块
2. 深度分析前端 React 代码
3. 汇总问题清单
4. 提供修复建议
