#![cfg(all(feature = "windows-service-mode", target_os = "windows"))]
use njupt_wifi_login_configuration::password::PasswordScope;
use std::path::PathBuf;
use windows_service::{
    service::{ServiceAccess, ServiceStartType, ServiceState},
    service_manager::{ServiceManager, ServiceManagerAccess},
};

use crate::fl;

use super::get_core_path;

pub struct WindowsServiceLauncher {
    core_path: PathBuf,
    service_name: String,
}

impl WindowsServiceLauncher {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let core_path = get_core_path()?;
        Ok(Self {
            core_path,
            service_name: "njupt_wifi_login".to_string(),
        })
    }
}

impl super::Launcher for WindowsServiceLauncher {
    fn name(&self) -> String {
        fl!("windows-service-launcher-name")
    }

    fn enable(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let status = runas::Command::new(self.core_path.as_path())
            .args(&["service", "--name", self.service_name.as_str(), "install"])
            .status()?;
        if !status.success() {
            return Err(format!("Failed to install the service: {}", status).into());
        }
        Ok(())
    }

    fn disable(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let status = runas::Command::new(self.core_path.as_path())
            .args(&["service", "--name", self.service_name.as_str(), "uninstall"])
            .status()?;
        if !status.success() {
            return Err(format!("Failed to uninstall the service: {}", status).into());
        }
        Ok(())
    }

    fn start(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let status = runas::Command::new(self.core_path.as_path())
            .args(&["service", "--name", self.service_name.as_str(), "start"])
            .status()?;
        if !status.success() {
            return Err(format!("Failed to start the service: {}", status).into());
        }
        Ok(())
    }

    fn stop(&self) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let status = runas::Command::new(self.core_path.as_path())
            .args(&["service", "--name", self.service_name.as_str(), "stop"])
            .status()?;
        if !status.success() {
            return Err(format!("Failed to stop the service: {}", status).into());
        }
        Ok(())
    }

    fn is_enabled(&self) -> Result<bool, Box<dyn std::error::Error + Sync + Send>> {
        let manager_access = ServiceManagerAccess::CONNECT;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
        let service =
            service_manager.open_service(self.service_name.as_str(), ServiceAccess::QUERY_CONFIG);
        if let Ok(service) = service {
            if service.query_config()?.start_type == ServiceStartType::AutoStart {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn is_running(&self) -> Result<bool, Box<dyn std::error::Error + Sync + Send>> {
        let manager_access = ServiceManagerAccess::CONNECT;
        let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
        let service =
            service_manager.open_service(self.service_name.as_str(), ServiceAccess::QUERY_STATUS);
        if let Ok(service) = service {
            match service.query_status()?.current_state {
                ServiceState::Stopped => {}
                ServiceState::StopPending => {}
                _ => return Ok(true),
            }
        }
        Ok(false)
    }

    fn is_password_scope_supported(&self, scope: &PasswordScope) -> bool {
        !matches!(scope, PasswordScope::CurrentUser)
    }
}
