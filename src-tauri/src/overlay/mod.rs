//! 悬浮窗模块。
//!
//! 把 Overlay 的三层职责拆成独立子模块，对齐项目已有的
//! `key_listener/`、`recorder/`、`output/` 目录组织：
//! - `seam`：`OverlayWindow` / `OverlaySink` / `OverlayState` / `OverlayError` 接口
//! - `manager`：`OverlayManager`，把状态意图翻译成窗口操作
//! - `tauri`：`TauriOverlayWindow`，Tauri 平台的 `OverlayWindow` adapter
//! - `activity`：`UserActivityClock`，自动淡出用的全局输入活动时钟
//!
//! Overlay module.
//!
//! Splits Overlay's three responsibilities into standalone submodules, mirroring the existing
//! `key_listener/` / `recorder/` / `output/` directory layout:
//! - `seam`: the `OverlayWindow` / `OverlaySink` / `OverlayState` / `OverlayError` interfaces
//! - `manager`: `OverlayManager`, translating state intents into window operations
//! - `tauri`: `TauriOverlayWindow`, the Tauri platform adapter of `OverlayWindow`
//! - `activity`: `UserActivityClock`, the global input-activity clock used by auto fade-out

pub mod activity;
pub mod manager;
pub mod seam;
pub mod tauri;
