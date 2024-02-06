#![cfg(all(feature = "windows-service-mode", target_os = "windows"))]
use std::{
    ffi::OsStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::task::AbortHandle;
use windows_service::{
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
};

use crate::app_events::AppEvents;

struct AbortHandleWrapper {
    handle: Option<AbortHandle>,
    stop_requested: bool,
}
impl AbortHandleWrapper {
    fn new() -> Self {
        Self {
            handle: None,
            stop_requested: false,
        }
    }
    fn register(&mut self, handle: AbortHandle) {
        if self.stop_requested {
            handle.abort();
        } else {
            self.handle.replace(handle);
        }
    }
    fn stop(&mut self) {
        self.stop_requested = true;
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

pub struct AppServiceEvents {
    status_handle: ServiceStatusHandle,
    abort_handle: Arc<Mutex<AbortHandleWrapper>>,
}
impl AppEvents for AppServiceEvents {
    fn on_started(&self) {
        let next_status = ServiceStatus {
            process_id: None,
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
        };
        self.status_handle.set_service_status(next_status).unwrap();
    }
    fn on_stopping(&self) {
        let next_status = ServiceStatus {
            process_id: None,
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StopPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
        };
        self.status_handle.set_service_status(next_status).unwrap();
    }
    fn on_stopped(&self) {
        let next_status = ServiceStatus {
            process_id: None,
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
        };
        self.status_handle.set_service_status(next_status).unwrap();
    }
    fn register_abort_handle(&mut self, handle: AbortHandle) {
        self.abort_handle.lock().unwrap().register(handle);
    }
}

impl AppServiceEvents {
    pub fn new(
        service_name: impl AsRef<OsStr>,
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let abort_handle = Arc::new(Mutex::new(AbortHandleWrapper::new()));
        let abort_handle_2 = abort_handle.clone();
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop => {
                    abort_handle.lock().unwrap().stop();
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };
        let status_handle = service_control_handler::register(service_name, event_handler)?;
        Ok(Self {
            status_handle,
            abort_handle: abort_handle_2,
        })
    }
}
