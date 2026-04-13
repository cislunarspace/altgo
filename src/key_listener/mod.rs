//! 按键监听器模块（跨平台）。
//!
//! 通过 `#[cfg(target_os)]` 条件编译为每个平台导出统一的类型别名 `PlatformListener`，
//! 实现静态分派，无需 trait 对象。
//!
//! - Linux：`xinput test-xi2`（XInput2 扩展）
//! - macOS：通过内联 Swift 脚本使用 CGEvent tap（需要辅助功能权限）
//! - Windows：PowerShell + `GetAsyncKeyState` 轮询

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub type PlatformListener = linux::X11Listener;

#[cfg(target_os = "macos")]
pub type PlatformListener = macos::MacOSListener;

#[cfg(target_os = "windows")]
pub type PlatformListener = windows::WindowsListener;

/// 按键事件。
#[derive(Debug)]
pub struct KeyEvent {
    /// 是否为按下事件
    pub pressed: bool,
}
