#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub type PlatformRecorder = linux::PulseRecorder;

#[cfg(target_os = "macos")]
pub type PlatformRecorder = macos::SoxRecorder;

#[cfg(target_os = "windows")]
pub type PlatformRecorder = windows::WindowsRecorder;
