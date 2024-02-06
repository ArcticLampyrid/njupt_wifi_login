mod desktop_launcher;
mod launcher_trait;
mod windows_service_launcher;
use std::{env, ffi::OsString, io, path::PathBuf};

pub use desktop_launcher::DesktopLauncher;
pub use launcher_trait::Launcher;
#[cfg(all(feature = "windows-service-mode", target_os = "windows"))]
pub use windows_service_launcher::WindowsServiceLauncher;

pub fn get_core_path() -> Result<PathBuf, io::Error> {
    env::current_exe().map(|mut path| {
        match path.extension() {
            Some(ext) => {
                let mut file_name = OsString::new();
                file_name.push("njupt_wifi_login.");
                file_name.push(ext);
                path.pop();
                path.push(file_name)
            }
            None => {
                path.pop();
                path.push("njupt_wifi_login")
            }
        }
        path
    })
}
