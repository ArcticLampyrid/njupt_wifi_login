use crate::app_events::AppEvents;
use crate::login::{self, get_network_status, send_login_request, WifiLoginError};
use crate::off_hours_cache::OffHoursCache;
use display_error_chain::ErrorChainExt;
use log::*;
use njupt_wifi_login_configuration::login_config::LoginConfig;
use rand::RngExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub enum ActionInfo {
    NetworkChanged(),
    ProactiveCheck(),
}

#[derive(Clone, Copy, Debug)]
enum CheckOutcome {
    Connected,
    Disconnected,
    AuthenticationUnknown,
    NetworkStatusError,
    LoginSucceeded,
    LoginFailed,
    LoginOffHours,
}

#[derive(Clone, Copy, Debug)]
struct ProactiveCheckConfig {
    base: Duration,
    backoff_enabled: bool,
    cap: Duration,
    jitter_percent: u64,
}

#[derive(Clone, Copy, Debug)]
struct ProactiveCheckState {
    consecutive_login_failures: u32,
    next_delay: Duration,
}

fn normalize_proactive_check_config(config: &LoginConfig) -> ProactiveCheckConfig {
    let base_seconds = match config.check_interval {
        0 => 0,
        interval if interval < 15 => {
            warn!(
                "Proactive check interval {} seconds is too short, using 15 seconds",
                interval
            );
            15
        }
        interval => interval,
    };
    let max_interval = config.login_failure_backoff.max_interval;
    if base_seconds > 0 && max_interval < base_seconds {
        warn!(
            "Login-failure backoff max interval {} seconds is below the proactive check interval {}; using {} seconds",
            max_interval,
            base_seconds,
            base_seconds
        );
    }
    let jitter_percent = config.login_failure_backoff.jitter_percent.min(100);
    if jitter_percent != config.login_failure_backoff.jitter_percent {
        warn!(
            "Login-failure backoff jitter {}% is above 100%; using 100%",
            config.login_failure_backoff.jitter_percent
        );
    }
    let cap_seconds = base_seconds.max(max_interval);
    ProactiveCheckConfig {
        backoff_enabled: config.login_failure_backoff.enabled,
        base: Duration::from_secs(base_seconds),
        cap: Duration::from_secs(cap_seconds),
        jitter_percent,
    }
}

fn next_failure_count(current: u32, outcome: CheckOutcome) -> u32 {
    match outcome {
        CheckOutcome::NetworkStatusError | CheckOutcome::LoginFailed => current.saturating_add(1),
        CheckOutcome::Connected
        | CheckOutcome::Disconnected
        | CheckOutcome::AuthenticationUnknown
        | CheckOutcome::LoginSucceeded
        | CheckOutcome::LoginOffHours => 0,
    }
}

fn select_delay(config: ProactiveCheckConfig, failures: u32) -> Duration {
    if !config.backoff_enabled || failures == 0 {
        return config.base;
    }
    let multiplier = 2_u64.saturating_pow(failures);
    let exponential_seconds = config.base.as_secs().saturating_mul(multiplier);
    let upper_seconds = exponential_seconds.min(config.cap.as_secs());
    let lower_seconds = config
        .base
        .as_secs()
        .max(upper_seconds.saturating_mul(100 - config.jitter_percent) / 100);
    if upper_seconds <= lower_seconds {
        return Duration::from_secs(upper_seconds);
    }
    Duration::from_secs(rand::rng().random_range(lower_seconds..=upper_seconds))
}

pub struct AppMain {
    config: LoginConfig,
    off_hours_cache: OffHoursCache,
}
impl AppMain {
    pub fn new(config: LoginConfig) -> AppMain {
        AppMain {
            config,
            off_hours_cache: OffHoursCache::new(),
        }
    }
    pub fn run(
        self,
        mut events: impl AppEvents,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            {
                if self.config.security.danger_accept_invalid_certs {
                    warn!("Danger: Accepting invalid certificates");
                }
                let (tx, rx) = mpsc::unbounded_channel::<ActionInfo>();
                #[cfg(target_os = "windows")]
                let _win32_connectivity_hint_listener_handle =
                    self.register_win32_connectivity_hint_listener(tx).await?; // there is an initial notification after registration
                #[cfg(not(target_os = "windows"))]
                let _ = tx.send(ActionInfo::NetworkChanged()); // initial check
                #[cfg(target_os = "linux")]
                let linux_network_listener_handle =
                    self.register_linux_network_listener(tx).await?;

                events.on_started();
                info!("Started");
                let proactive_check_config = normalize_proactive_check_config(&self.config);
                let event_loop_handle =
                    tokio::spawn(async move { self.event_loop(rx, proactive_check_config).await });
                events.register_abort_handle(event_loop_handle.abort_handle());
                if let Ok(Err(err)) = event_loop_handle.await {
                    error!("Event loop error: {}", err.as_ref().chain());
                }
                info!("Stopping");
                events.on_stopping();

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
                tx.send(ActionInfo::NetworkChanged()).unwrap();
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
        let handle = LinuxNetworkListenerHandle::register(
            move || {
                tx.send(ActionInfo::NetworkChanged()).unwrap();
            },
            self.config.interface.clone(),
        )?;
        Ok(handle)
    }

    async fn event_loop(
        mut self,
        mut rx: UnboundedReceiver<ActionInfo>,
        proactive_check_config: ProactiveCheckConfig,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        let mut last_check_at: Option<Instant> = None;
        let dns_resolver = login::new_dns_resolver(self.config.interface.clone());
        let mut proactive_check_state = ProactiveCheckState {
            consecutive_login_failures: 0,
            next_delay: proactive_check_config.base,
        };
        if proactive_check_config.base.is_zero() {
            info!("Proactive check is disabled");
        }
        let mut next_proactive_deadline = {
            let duration = proactive_check_state.next_delay;
            (!duration.is_zero()).then_some(Instant::now() + duration)
        };
        loop {
            // Firstly try to receive an action without blocking,
            // if there is no action, then wait for either a new action or the next proactive check deadline.
            // When channel is closed, return Ok(()) to exit the loop.
            let action = match rx.try_recv() {
                Ok(action) => action,
                Err(mpsc::error::TryRecvError::Empty) => {
                    let now = Instant::now();
                    let off_hours_deadline = {
                        let expiration = self.off_hours_cache.expiration();
                        (!expiration.is_zero()).then_some(now + expiration)
                    };
                    let timer_deadline = off_hours_deadline.or(next_proactive_deadline);
                    match timer_deadline {
                        // If the deadline is already passed, we should do a proactive check immediately.
                        Some(deadline) if deadline <= now => ActionInfo::ProactiveCheck(),
                        // If there is a deadline in the future, wait for either a new action or the deadline.
                        Some(deadline) => {
                            info!(
                                "Waiting for next action or proactive check timer (after {:?})",
                                deadline.duration_since(now)
                            );
                            let proactive_timer =
                                tokio::time::sleep_until(tokio::time::Instant::from_std(deadline));
                            tokio::pin!(proactive_timer);
                            tokio::select! {
                                action = rx.recv() => match action {
                                    Some(action) => action,
                                    None => return Ok(()),
                                },
                                _ = &mut proactive_timer => ActionInfo::ProactiveCheck(),
                            }
                        }
                        // If there is no deadline, wait for a new action indefinitely.
                        None => match rx.recv().await {
                            Some(action) => action,
                            None => return Ok(()),
                        },
                    }
                }
                Err(mpsc::error::TryRecvError::Disconnected) => return Ok(()),
            };

            // Debouncing, make a minimum interval of 5 seconds between checks
            // New check requests within 5 seconds of the last check will be delayed
            let check_at = Instant::now();
            if let Some(last_check_at) = last_check_at {
                if check_at.duration_since(last_check_at) < Duration::from_secs(5) {
                    next_proactive_deadline = Some(last_check_at + Duration::from_secs(5));
                    // Reset the off-hours cache
                    // If the original check was triggered by timer, there should be no effect.
                    //     (off-hours-cache is already considered when scheduling the next proactive check)
                    // If the original check was triggered by network change, it's expected to reset the off-hours cache
                    self.off_hours_cache.clear();
                    info!(
                        "Debouncing: delaying check for {} seconds",
                        next_proactive_deadline
                            .unwrap()
                            .duration_since(check_at)
                            .as_secs()
                    );
                    continue;
                }
            }

            last_check_at = Some(check_at);
            let outcome = self.check_network_and_login(dns_resolver.clone()).await;
            match outcome {
                CheckOutcome::LoginSucceeded => self.off_hours_cache.clear(),
                CheckOutcome::LoginOffHours => self.off_hours_cache.set(),
                _ => {}
            }
            let consecutive_login_failures =
                next_failure_count(proactive_check_state.consecutive_login_failures, outcome);
            let next_delay = select_delay(proactive_check_config, consecutive_login_failures);
            proactive_check_state = ProactiveCheckState {
                consecutive_login_failures,
                next_delay,
            };
            next_proactive_deadline = {
                let duration = proactive_check_state.next_delay;
                (!duration.is_zero()).then_some(Instant::now() + duration)
            };
            let trigger_name = match action {
                ActionInfo::NetworkChanged() => "network-change",
                ActionInfo::ProactiveCheck() => "proactive-check",
            };
            info!(
                "Completed check with outcome {:?}; trigger: {}; consecutive login failures: {}",
                outcome, trigger_name, proactive_check_state.consecutive_login_failures
            );
        }
    }

    async fn check_network_and_login(
        &self,
        dns_resolver: Arc<crate::dns::resolver::CustomTrustDnsResolver>,
    ) -> CheckOutcome {
        let network_status = get_network_status(
            self.config.interface.as_deref(),
            dns_resolver.clone(),
            &self.config.security,
        )
        .await;
        let network_status = match network_status {
            Ok(network_status) => network_status,
            Err(err) => {
                error!("Failed to get network status: {}", err.chain());
                return CheckOutcome::NetworkStatusError;
            }
        };
        match network_status {
            login::NetworkStatus::Connected => CheckOutcome::Connected,
            login::NetworkStatus::Disconnected => CheckOutcome::Disconnected,
            login::NetworkStatus::AuthenticationUnknown => CheckOutcome::AuthenticationUnknown,
            login::NetworkStatus::AuthenticationNJUPT(ap_info) => {
                info!("Start to login: {:?}", ap_info);
                match send_login_request(
                    self.config.interface.as_deref(),
                    dns_resolver,
                    &self.config.security,
                    &self.config.credential,
                    &ap_info,
                )
                .await
                {
                    Ok(_) => {
                        info!("Connected");
                        CheckOutcome::LoginSucceeded
                    }
                    Err(WifiLoginError::OffHours()) => {
                        error!("Failed to connect: off hours");
                        CheckOutcome::LoginOffHours
                    }
                    Err(err) => {
                        error!("Failed to connect: {}", err.chain());
                        CheckOutcome::LoginFailed
                    }
                }
            }
        }
    }
}
