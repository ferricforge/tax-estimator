#[cfg(target_os = "linux")]
pub mod linux_theme;
#[cfg(target_os = "macos")]
pub mod macos_theme;

#[cfg(target_os = "linux")]
pub use linux_theme::apply_linux_system_theme;
#[cfg(target_os = "macos")]
pub use macos_theme::apply_macos_system_theme;
