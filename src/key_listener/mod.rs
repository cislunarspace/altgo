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

/// Key event from the listener.
#[derive(Debug)]
pub struct KeyEvent {
    pub pressed: bool,
}
