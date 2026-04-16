# altgo 全面改进计划

## 目标
从语音识别质量、文本润色效果、按键响应速度、整体交互体验四个维度全面提升 altgo 使用体验。

## 任务列表

### A. 语音识别质量
- [x] 1. 暴露 Whisper API `temperature` 参数到配置文件，默认 0（最确定性）
- [x] 2. 暴露 Whisper API `prompt` 参数到配置文件，用于提供领域词汇/上下文提示，提升识别准确率
- [ ] 3. 暴露 Whisper API `response_format` 参数，支持 `verbose_json` 以获取更详细信息

### B. 文本润色效果
- [x] 4. 改造 polisher 提示词系统：根据 `config.language` 动态生成多语言 prompt，而非硬编码中文
- [x] 5. 将 polisher 的 `temperature` 参数暴露到配置文件
- [x] 6. 润色提示词优化：区分"口语转书面语"和"纠错"两种模式，提供更精细的控制（通过多语言 prompt 实现）
- [x] 7. 支持 polisher 自定义 system prompt（通过配置文件覆盖默认 prompt）

### C. 按键响应/延迟
- [x] 8. 将 `min_press_duration`（100ms）从硬编码改为可配置
- [x] 9. Windows 键监听优化：降低默认 poll_interval 从 50ms 到 30ms，提升响应速度
- [ ] 10. 修复录音窗口关闭为 no-op 的问题，实现即时关闭

### D. 整体交互体验
- [x] 11. 优化状态机超时参数的默认值：long_press_threshold 从 300ms 降到 200ms
- [ ] 12. 改进错误提示：polisher 失败时显示更友好的提示（而非原始错误）
- [x] 13. 修复 polisher 认证错误检测（从字符串匹配改为 HTTP 状态码判断）

### E. 配置/代码质量
- [x] 14. 统一所有硬编码参数为可配置项
- [x] 15. 更新 configs/altgo.toml 模板，包含所有新增配置项和注释说明

## 执行优先级
- **第一批（核心质量）**: 1, 2, 4, 6, 7 — 直接影响输出质量
- **第二批（响应速度）**: 8, 9, 11 — 改善按键手感
- **第三批（体验打磨）**: 3, 5, 10, 12, 13 — 锦上添花
- **第四批（收尾）**: 14, 15 — 配置模板更新

## 备注
- 每项改动都需通过 `cargo fmt -- --check`、`cargo clippy -- -D warnings`、`cargo test`
- 配置项新增一律使用 `serde(default)`，保持向后兼容
- 不改变现有 API/协议，只扩展参数
