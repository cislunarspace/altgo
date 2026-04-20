# 文档全面更新设计方案

**日期：** 2026-04-19
**状态：** 已批准
**范围：** README.md、CLAUDE.md、CONTRIBUTING.md

---

## 概述

本次更新旨在修复文档过时问题，确保与代码库当前状态一致。主要变更：

- 移除 CLI 模式相关内容（`src/bin/cli.rs` 已不存在）
- 更新路径引用（`src/` → `src-tauri/src/`）
- 新增已实现但未文档化的模块
- 保持中英混合的现有文档风格

---

## 详细变更

### 1. README.md

**文件：** `README.md`

**变更：**

1. **移除 CLI 模式内容**
   - 删除 "项目包含两种运行模式" 表格（CLI + Tauri GUI）
   - 删除 `src/bin/cli.rs` 相关说明
   - 简化为纯 Tauri GUI 描述

2. **更新架构描述**
   ```
   按键事件 → 状态机 → 录音 → ASR 转写 → LLM 润色 → 剪贴板 + 通知
   ```

3. **更新构建命令**
   - `cargo build --release` → `cargo build --release --manifest-path=src-tauri/Cargo.toml`
   - `cargo test` → `cargo test --manifest-path=src-tauri/Cargo.toml`
   - 新增 `cargo tauri dev` / `cargo tauri build` 说明
   - 新增前端依赖安装说明 (`cd frontend && npm install`)

4. **新增内容**
   - AppImage 安装说明（Linux）
   - macOS ARM64 构建说明
   - 系统托盘功能说明

### 2. CLAUDE.md

**文件：** `README.md`（项目根目录）

**变更：**

1. **修复路径引用**
   - 所有 `src/` → `src-tauri/src/`
   - 示例：`src/lib.rs` → `src-tauri/src/lib.rs`

2. **更新模块列表**

   **新增模块：**
   - `pipeline.rs` — 核心处理管道（转写 + 润色），调用方负责输出
   - `model.rs` — whisper.cpp GGML 模型管理（下载、切换、存储）
   - `tray.rs` — 系统托盘配置（显示窗口、退出菜单）
   - `resource.rs` — 资源文件管理（待确认用途）

   **现有模块（更新描述）：**
   - `lib.rs` — Tauri 应用入口，`AppState` 结构，运行循环设置
   - `cmd.rs` — Tauri 命令（get_config、save_config、start_pipeline、stop_pipeline、get_status、copy_text、hide_overlay）
   - `config.rs` — TOML 配置加载，`serde(default)` 注解，`ALTGO_*_API_KEY` 环境变量覆盖
   - `state_machine.rs` — 5 状态枚举（Idle、PotentialPress、Recording、WaitSecondClick、ContinuousRecording）
   - `audio.rs` — 线程安全 PCM 缓冲，`Mutex<Vec<u8>>`，WAV 编解码
   - `transcriber.rs` — `WhisperApi`（HTTP multipart）+ `LocalWhisper`（subprocess whisper-cli）
   - `polisher.rs` — LLM 润色，4 级别（none/light/medium/heavy），指数退避重试
   - `key_listener/` — 平台按键监听
   - `recorder/` — 平台录音
   - `output/` — 平台剪贴板 + 通知

3. **新增前端结构文档**
   ```
   frontend/src/
   ├── App.tsx              # 应用入口
   ├── main.tsx             # React 渲染入口
   ├── overlay.tsx          # 浮动窗口组件
   ├── overlay.css          # 浮动窗口样式
   ├── components/
   │   ├── ui/              # UI 基础组件（Input、Button、Card）
   │   ├── Layout.tsx       # 布局组件
   │   └── StatusIndicator.tsx # 状态指示器
   ├── pages/
   │   ├── Home.tsx         # 首页
   │   └── Settings.tsx      # 设置页
   ├── hooks/
   │   └── useTauri.ts      # Tauri 集成钩子
   ├── i18n/                # 国际化
   └── styles/               # CSS 样式
   ```

4. **更新构建命令**
   - 使用 `--manifest-path=src-tauri/Cargo.toml`
   - 新增 Tauri GUI 模式命令
   - 新增 `make build` / `make install`

5. **更新测试说明**
   - 平台模块仅有构造/烟雾测试

### 3. CONTRIBUTING.md

**文件：** `CONTRIBUTING.md`

**变更：**

1. **移除 CLI 引用**
   - 删除 `src/bin/cli.rs` 相关说明

2. **更新检查命令**
   - `cargo fmt -- --check` → `cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check`
   - `cargo clippy -- -D warnings` → `cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings`

3. **更新文件大小限制**
   - 函数 < 50 行（保留）
   - 文件 < 800 行 → < 1000 行（适应 Rust + 注释）

---

## 实施顺序

1. 更新 README.md
2. 更新 CLAUDE.md
3. 更新 CONTRIBUTING.md
4. 提交变更

---

## 风险与注意事项

- **CHANGELOG.md** 未列入更新范围，如需同步更新 v1.0.0 之后的功能（如 AppImage、Wayland 支持），需单独处理
- **docs-site/** 文档（Docusaurus 站点）未列入范围，如需同步更新需单独处理
- 保持现有中英混合文档风格，不做语言统一
