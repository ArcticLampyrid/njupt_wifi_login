mod desktop_launcher;
mod launcher_trait;
use std::{env, ffi::OsString, io, path::PathBuf};

pub use desktop_launcher::DesktopLauncher;
pub use launcher_trait::Launcher;

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
