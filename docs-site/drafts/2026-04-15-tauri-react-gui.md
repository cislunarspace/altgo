# Tauri + React 桌面应用改造

## 目标

将 altgo 从 eframe/egui GUI 迁移为 **Tauri v2 + React + TypeScript** 架构，实现：
- Windows 下无控制台窗口的桌面应用
- 完整的应用窗口（录音状态、转录历史、设置页）
- 系统托盘支持（后台运行、右键菜单）
- React 前端 + Rust 后端，通过 Tauri IPC 通信

## 架构总览

```
altgo/
├── src-tauri/              # Rust 后端（Tauri）
│   ├── Cargo.toml          # Tauri 依赖 + 原有依赖
│   ├── src/
│   │   ├── lib.rs          # Tauri 入口（commands、tray、插件注册）
│   │   └── main.rs         # Tauri main（仅调用 lib）
│   ├── capabilities/       # Tauri v2 权限配置
│   ├── icons/              # 应用图标
│   └── tauri.conf.json     # Tauri 配置
├── frontend/               # React 前端
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── index.html
│   └── src/
│       ├── main.tsx        # React 入口
│       ├── App.tsx         # 主布局 + 路由
│       ├── pages/
│       │   ├── Home.tsx    # 主页：录音状态、最新转录
│       │   ├── History.tsx # 转录历史
│       │   └── Settings.tsx # 设置页
│       ├── components/     # 通用组件
│       ├── hooks/          # Tauri IPC hooks
│       ├── i18n/           # 国际化
│       └── styles/         # CSS
├── src/                    # 原有 Rust 核心逻辑（保留为 lib crate）
│   ├── lib.rs              # pub mod 导出
│   ├── config.rs
│   ├── audio.rs
│   ├── transcriber.rs
│   ├── polisher.rs
│   ├── pipeline.rs
│   ├── state_machine.rs
│   ├── key_listener/
│   ├── recorder/
│   └── output/
├── Cargo.toml              # workspace 根（或直接使用 src-tauri/Cargo.toml）
└── configs/altgo.toml
```

## 任务列表

### 阶段 A：项目结构搭建

- [x] 1. 初始化 Tauri v2 项目骨架（`npm create tauri-app@latest` 或手动创建结构）
- [x] 2. 配置 `src-tauri/Cargo.toml`：添加 tauri 依赖，引入原有核心模块
- [x] 3. 配置 `src-tauri/tauri.conf.json`：窗口设置（无装饰/标题栏）、系统托盘、打包配置
- [x] 4. 配置 `frontend/` Vite + React + TypeScript 项目（package.json、vite.config.ts、tsconfig）
- [x] 5. 删除旧的 `gui` feature 及 `src/gui/` 目录（eframe/egui 代码）

### 阶段 B：Rust 后端 — Tauri Commands

- [x] 6. 将 `src/main.rs` 的核心逻辑重构为 library（`src/lib.rs` 导出所有模块）
- [x] 7. 实现 `src-tauri/src/lib.rs`：注册 Tauri commands、system tray、插件
- [x] 8. 实现 `get_config` command：返回当前配置给前端
- [x] 9. 实现 `save_config` command：接收前端配置并写入 TOML
- [x] 10. 实现音频管道 command：`start_pipeline`（启动按键监听+录音管道）、`stop_pipeline`
- [x] 11. 实现 `get_status` command：返回当前录音/处理状态
- [ ] 12. 实现 `get_history` / `clear_history` commands：转录历史管理（暂缓）
- [x] 13. 配置 Tauri 事件系统：后端通过 `app.emit()` 推送状态变更（录音开始/结束/转录完成）到前端

### 阶段 C：React 前端

- [x] 14. 搭建前端基础框架：路由（Home / Settings）、布局组件、全局样式（暗色主题）
- [x] 15. 实现 i18n 模块（复用现有 zh/en 翻译 key，迁移为 JSON 格式）
- [x] 16. 实现 Home 页面：录音状态指示器、最新转录结果展示、快捷键提示
- [x] 17. 实现 Settings 页面：转写设置、润色设置、快捷键设置、UI 语言切换
- [x] 18. 实现 Tauri IPC hooks：`useConfig()`、`useStatus()`、`useTauriEvent()`
- [ ] 19. 实现转录历史页面（History）：列表展示、复制、清空（暂缓）

### 阶段 D：系统托盘 + 收尾

- [x] 20. 配置系统托盘：图标、tooltip、右键菜单（显示窗口/设置/退出）
- [ ] 21. 实现最小化到托盘、关闭到托盘逻辑
- [x] 22. Windows 子系统配置：确保无控制台窗口（Tauri 默认处理）
- [ ] 23. 更新 `Makefile` / CI 配置：新增 `make tauri-build`、更新 GitHub Actions
- [ ] 24. 更新 CLAUDE.md 文档：反映新架构
- [x] 25. 清理：移除 eframe/egui 依赖、更新 .gitignore、验证 `cargo clippy` 和 `cargo test`

## 技术选型

| 组件 | 选择 | 理由 |
|------|------|------|
| 桌面框架 | Tauri v2 (2.x) | Rust 原生、轻量（~5MB）、内置 tray/通知支持 |
| 前端框架 | React 18 + TypeScript | 用户要求 |
| 构建工具 | Vite 6 | Tauri 官方推荐，HMR 极快 |
| UI 组件库 | shadcn/ui (Radix + Tailwind) | 轻量、可定制、暗色主题友好 |
| CSS 方案 | Tailwind CSS v4 | 与 shadcn/ui 配合 |
| 状态管理 | Zustand | 轻量，适合中小项目 |
| 路由 | React Router v7 | 标准选择 |
| i18n | 自定义 JSON + hook | 项目规模小，不需要 i18next |

## 风险与约束

1. **Tauri v2 稳定性**：v2 已发布正式版，API 稳定，但部分插件可能仍在 alpha
2. **Windows webview**：Tauri v2 使用 Edge WebView2（Windows 10+ 自带），旧系统需手动安装
3. **录音权限**：Tauri 的 webview 内无法直接录音，录音由 Rust 后端通过 subprocess 处理（复用现有方案）
4. **体积变化**：Tauri 二进制约 3-5MB，加上前端资源约 1-2MB，总包约 5-7MB（vs 当前纯 Rust ~2MB）
5. **构建复杂度**：需要 Node.js + Rust 工具链，CI 需同时安装 npm 和 cargo
6. **向后兼容**：CLI 模式保留，`--no-gui` 标志仍然可用

## 备注

- 现有 `src/gui/` 下的 eframe/egui 代码将被完全替换
- 核心管道逻辑（key_listener、recorder、transcriber、polisher、state_machine）保持不变
- 原有 Rust 测试应继续通过
- 优先实现 Windows 平台，Linux 后续适配
