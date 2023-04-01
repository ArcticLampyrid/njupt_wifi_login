#[cfg_attr(windows, path = "windows.rs")]
#[cfg_attr(target_os = "linux", path = "linux.rs")]
mod network_changed_impl;

pub use network_changed_impl::NetworkChangedListener;
