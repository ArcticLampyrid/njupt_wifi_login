#![cfg(all(feature = "windows-service-mode", target_os = "windows"))]
use std::{
    env,
    ffi::{OsStr, OsString},
};

use clap::{Args, Subcommand};
use njupt_wifi_login_configuration::login_config::LoginConfig;
use windows_service::{
    define_windows_service,
    service::ServiceDependency,
    service::{
        ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceState,
        ServiceType,
    },
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

use crate::{app_main::AppMain, app_service_events::AppServiceEvents};

struct ServiceGlobals {
    config: LoginConfig,
    service_name: String,
}
static mut SERVICE_GLOBALS: Option<ServiceGlobals> = None;
/// Entrypoint for the Windows service.
pub fn service_main(_arguments: Vec<OsString>) {
    let globals = unsafe { SERVICE_GLOBALS.take().unwrap() };
    let app = AppMain::new(globals.config);
    app.run(AppServiceEvents::new(globals.service_name).unwrap())
        .unwrap();
}
define_windows_service!(ffi_service_main, service_main);

#[derive(Args, Clone, Debug)]
pub struct ServiceCommand {
    /// Name of the service.
    /// If not provided, the service will be named "njupt_wifi_login".
    #[arg(short, long)]
    name: Option<String>,
    #[command(subcommand)]
    subcommand: ServiceSubCommand,
}

#[derive(Subcommand, Clone, Debug)]
pub enum ServiceSubCommand {
    /// Install the service.
    Install,
    /// Uninstall the service.
    Uninstall,
    /// Start the service.
    Start,
    /// Stop the service.
    Stop,
    #[command(hide = true)]
    Main,
}

pub fn handle_service_command(
    command: ServiceCommand,
    my_config: LoginConfig,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let service_name = command
        .name
        .unwrap_or_else(|| "njupt_wifi_login".to_string());
    match command.subcommand {
        ServiceSubCommand::Install => {
            let manager_access =
                ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
            let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
            let service_info = ServiceInfo {
                name: OsString::from(service_name.as_str()),
                display_name: format!("NJUPT WiFi Login Service ({})", service_name).into(),
                service_type: ServiceType::OWN_PROCESS,
                start_type: ServiceStartType::AutoStart,
                error_control: ServiceErrorControl::Normal,
                executable_path: env::current_exe()?,
                launch_arguments: vec![
                    OsString::from("service"),
                    OsString::from("--name"),
                    OsString::from(service_name.as_str()),
                    OsString::from("main"),
                ],
                dependencies: vec![ServiceDependency::Service(OsString::from("nsi"))],
                account_name: None,
                account_password: None,
            };
            if let Ok(service) =
                service_manager.open_service(service_name, ServiceAccess::CHANGE_CONFIG)
            {
                service.change_config(&service_info)?;
            } else {
                service_manager.create_service(&service_info, ServiceAccess::CHANGE_CONFIG)?;
            }
        }
        ServiceSubCommand::Uninstall => {
            let manager_access = ServiceManagerAccess::CONNECT;
            let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
            let service = service_manager.open_service(
                service_name.as_str(),
                ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
            )?;
            // The service will be marked for deletion,
            // but it will not be deleted until it is stopped and all handles are closed.
            service.delete()?;
            if service.query_status()?.current_state != ServiceState::Stopped {
                service.stop()?;
            }
        }
        ServiceSubCommand::Start => {
            let manager_access = ServiceManagerAccess::CONNECT;
            let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
            let service =
                service_manager.open_service(service_name.as_str(), ServiceAccess::START)?;
            service.start(&[] as &[&OsStr])?;
        }
        ServiceSubCommand::Stop => {
            let manager_access = ServiceManagerAccess::CONNECT;
            let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;
            let service =
                service_manager.open_service(service_name.as_str(), ServiceAccess::STOP)?;
            service.stop()?;
        }
        ServiceSubCommand::Main => {
            let globals = ServiceGlobals {
                config: my_config,
                service_name: service_name.to_string(),
            };
            unsafe { SERVICE_GLOBALS = Some(globals) };
            service_dispatcher::start(service_name, ffi_service_main)?;
        }
    }
    Ok(())
}
