use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use njupt_wifi_login_configuration::password::PasswordScope;
use sysinfo::{ProcessRefreshKind, RefreshKind, Signal, System, UpdateKind};

use crate::fl;

use super::get_core_path;

pub struct DesktopLauncher {
    core_path: PathBuf,
    auto_launch: AutoLaunch,
}

impl DesktopLauncher {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let core_path = get_core_path()?;
        let auto_launch = AutoLaunchBuilder::new()
            .set_app_name("njupt_wifi_login")
            .set_app_path(core_path.to_string_lossy().as_ref())
            .set_use_launch_agent(true)
            .build()?;
        Ok(Self {
            core_path,
            auto_launch,
        })
    }

    fn for_process<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&sysinfo::Process) -> R,
    {
        let system = System::new_with_specifics(
            RefreshKind::new().with_processes(
                ProcessRefreshKind::new()
                    .with_exe(UpdateKind::Always)
                    .with_cmd(UpdateKind::Always),
            ),
        );

        for process in system.processes().values() {
            if let Some(exe) = process.exe() {
                if path_equals(exe, &self.core_path) {
                    // ignore process under service mode
                    if process.cmd().iter().any(|x| x == "service") {
                        continue;
                    }
                    return Some(f(process));
                }
            }
        }

        None
    }
}

fn path_equals<P1: AsRef<Path>, P2: AsRef<Path>>(path1: P1, path2: P2) -> bool {
    if let (Ok(path1), Ok(path2)) = (fs::canonicalize(path1), fs::canonicalize(path2)) {
        path1 == path2
    } else {
        false
    }
}

impl super::Launcher for DesktopLauncher {
    fn name(&self) -> String {
        fl!("desktop-launcher-name")
    }

    fn enable(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        self.auto_launch.enable()?;
        Ok(())
    }

    fn disable(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        self.auto_launch.disable()?;
        Ok(())
    }

    fn start(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        Command::new(self.core_path.as_path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        Ok(())
    }

    fn stop(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        self.for_process(|p| {
            if p.kill_with(Signal::Term).is_none() {
                // If SIGTERM is not supported, use SIGKILL
                p.kill();
            }
        });
        Ok(())
    }

    fn is_enabled(&self) -> Result<bool, Box<dyn std::error::Error + Sync + Send>> {
        Ok(self.auto_launch.is_enabled()?)
    }

    fn is_running(&self) -> Result<bool, Box<dyn std::error::Error + Sync + Send>> {
        Ok(self.for_process(|_| ()).is_some())
    }

    fn is_password_scope_supported(&self, _scope: &PasswordScope) -> bool {
        true
    }
}
