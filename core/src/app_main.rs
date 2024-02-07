use crate::app_events::AppEvents;
use crate::login::{self, get_network_status, send_login_request, WifiLoginError};
use crate::off_hours_cache::OffHoursCache;
use log::*;
use njupt_wifi_login_configuration::login_config::LoginConfig;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub enum ActionInfo {
    CheckAndLogin(),
}

pub struct AppMain {
    config: LoginConfig,
    off_hours_cache: Arc<Mutex<OffHoursCache>>,
}
impl AppMain {
    pub fn new(config: LoginConfig) -> AppMain {
        AppMain {
            config,
            off_hours_cache: Arc::new(Mutex::new(OffHoursCache::new())),
        }
    }
    pub fn run(
        self,
        mut events: impl AppEvents,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            {
                let (tx, rx) = mpsc::unbounded_channel::<ActionInfo>();
                let regular_check_handle = self.register_regular_check(tx.clone()).await?;
                #[cfg(target_os = "windows")]
                let _win32_connectivity_hint_listener_handle =
                    self.register_win32_connectivity_hint_listener(tx).await?; // there is an initial notification after registration
                #[cfg(not(target_os = "windows"))]
                let _ = tx.send(ActionInfo::CheckAndLogin()); // initial check
                #[cfg(target_os = "linux")]
                let linux_network_listener_handle =
                    self.register_linux_network_listener(tx).await?;

                events.on_started();
                info!("Started");
                let event_loop_handle = tokio::spawn(async move { self.event_loop(rx).await });
                events.register_abort_handle(event_loop_handle.abort_handle());
                if let Ok(Err(err)) = event_loop_handle.await {
                    error!("Event loop error: {}", err);
                }
                info!("Stopping");
                events.on_stopping();

                regular_check_handle.abort();
                let _ = regular_check_handle.await;

                #[cfg(target_os = "linux")]
                {
                    linux_network_listener_handle.abort();
                    linux_network_listener_handle.join().await;
                }
            }
            events.on_stopped();

            Ok(())
        })
    }
    async fn register_regular_check(
        &self,
        tx: UnboundedSender<ActionInfo>,
    ) -> Result<JoinHandle<()>, Box<dyn std::error::Error + Sync + Send>> {
        let off_hours_cache = self.off_hours_cache.clone();
        let join_handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(20 * 60)).await;
            while !tx.is_closed() {
                let expiration = off_hours_cache.lock().await.expiration();
                if expiration.is_zero() {
                    if tx.send(ActionInfo::CheckAndLogin()).is_err() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(20 * 60)).await;
                } else {
                    tokio::time::sleep(std::cmp::min(expiration, Duration::from_secs(20 * 60)))
                        .await;
                }
            }
        });
        Ok(join_handle)
    }

    #[cfg(target_os = "windows")]
    async fn register_win32_connectivity_hint_listener(
        &self,
        tx: UnboundedSender<ActionInfo>,
    ) -> Result<
        crate::win32_network_connectivity_hint_changed::NetworkConnectivityHintChangedHandle<
            'static,
        >,
        Box<dyn std::error::Error + Sync + Send>,
    > {
        use crate::win32_network_connectivity_hint_changed::NetworkConnectivityHintChangedHandle;
        use windows::Win32::Networking::WinSock::{
            NetworkConnectivityLevelHintConstrainedInternetAccess,
            NetworkConnectivityLevelHintLocalAccess, NL_NETWORK_CONNECTIVITY_HINT,
        };
        let listener = move |connectivity_hint: NL_NETWORK_CONNECTIVITY_HINT| {
            info!(
                "ConnectivityLevel = {}",
                connectivity_hint.ConnectivityLevel.0
            );
            if connectivity_hint.ConnectivityLevel
                == NetworkConnectivityLevelHintConstrainedInternetAccess
                || connectivity_hint.ConnectivityLevel == NetworkConnectivityLevelHintLocalAccess
            {
                tx.send(ActionInfo::CheckAndLogin()).unwrap();
            }
        };
        let handle = NetworkConnectivityHintChangedHandle::register(listener, true)?;
        Ok(handle)
    }

    #[cfg(target_os = "linux")]
    async fn register_linux_network_listener(
        &self,
        tx: UnboundedSender<ActionInfo>,
    ) -> Result<
        crate::linux_network_listener::LinuxNetworkListenerHandle,
        Box<dyn std::error::Error + Sync + Send>,
    > {
        use crate::linux_network_listener::LinuxNetworkListenerHandle;
        let handle = LinuxNetworkListenerHandle::register(move || {
            tx.send(ActionInfo::CheckAndLogin()).unwrap();
        })?;
        Ok(handle)
    }

    async fn event_loop(
        &self,
        mut rx: UnboundedReceiver<ActionInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let mut last_check_at: Option<std::time::Instant> = None;

        while let Some(action) = rx.recv().await {
            match action {
                ActionInfo::CheckAndLogin() => {
                    {
                        // debounce
                        let check_at = std::time::Instant::now();
                        if let Some(last_check_at) = last_check_at {
                            if check_at.duration_since(last_check_at) < Duration::from_secs(5) {
                                continue;
                            }
                        }
                        last_check_at = Some(check_at);
                    }

                    info!("Start to check network status");
                    let network_status = get_network_status().await;
                    info!("Network status: {:?}", network_status);
                    if let login::NetworkStatus::AuthenticationNJUPT(ap_info) = network_status {
                        info!("Start to login");
                        match send_login_request(&self.config.credential, &ap_info).await {
                            Ok(_) => {
                                info!("Connected");
                                self.off_hours_cache.lock().await.clear();
                            }
                            Err(err) => {
                                error!("Failed to connect: {}", err);
                                if let WifiLoginError::OffHours() = err {
                                    self.off_hours_cache.lock().await.set();
                                }
                            }
                        };
                    }
                }
            }
        }
        Ok(())
    }
}
