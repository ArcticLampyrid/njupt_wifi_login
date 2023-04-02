#![cfg_attr(windows, windows_subsystem = "windows")]

mod login;
mod network_changed;

use anyhow::Error;
use once_cell::sync::Lazy;
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use std::{env, time::Duration};
use std::{path::PathBuf, time::Instant};
use tracing::{error, info, subscriber, trace, Level};
use tracing_subscriber::{fmt, prelude::*, FmtSubscriber};

use login::login;
use network_changed::NetworkChangedListener;

use crate::login::LoginError;

static CONFIG_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = env::current_exe().unwrap();
    path.pop();
    path.push("njupt_wifi.yml");
    path
});
static LOG_DIRECTORY: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = env::current_exe().unwrap();
    path.pop();
    path.push("logs");
    path
});
static LOG_FILENAME: &str = "njupt_wifi.log";

const TIMEOUT_DURATION: Duration = Duration::from_secs(3);
const DEBOUNCE_DURATION: Duration = Duration::from_secs(3);
const MAX_TRY_COUNT: usize = 3;

#[derive(Serialize, Deserialize, Debug)]
pub struct Credential {
    userid: String,
    password: String,
    isp: IspType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum IspType {
    EDU,
    CMCC,
    CT,
}

fn read_config() -> Result<Credential, Error> {
    let f = std::fs::File::open(CONFIG_PATH.as_path())?;
    let credential: Credential = serde_yaml::from_reader(f)?;
    Ok(credential)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let file_appender = tracing_appender::rolling::daily(LOG_DIRECTORY.as_path(), LOG_FILENAME);
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    subscriber::set_global_default(
        FmtSubscriber::builder()
            .with_max_level(if cfg!(debug_assertions) {
                Level::TRACE
            } else {
                Level::INFO
            })
            .finish()
            .with(
                fmt::Layer::default()
                    .with_ansi(false)
                    .with_writer(file_writer),
            ),
    )?;

    let credential = read_config().map_err(|error| {
        error!("Failed to read config: {error}");
        error
    })?;

    let (_listener, mut rx) = NetworkChangedListener::listen()?;
    info!("Start to listen to network change");

    let mut debounce_begin = Instant::now() - DEBOUNCE_DURATION;
    while let Some(()) = rx.recv().await {
        if debounce_begin.elapsed() < DEBOUNCE_DURATION {
            trace!("Debounced");
            continue;
        }
        info!("Network changed");
        let mut try_count = 1;
        while try_count <= MAX_TRY_COUNT {
            info!("Start to login (try {try_count}/{MAX_TRY_COUNT})");
            // reusing client among different network may fail
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(TIMEOUT_DURATION)
                .connect_timeout(TIMEOUT_DURATION)
                .redirect(Policy::none())
                .build()?;
            match login(&client, &credential).await {
                Ok(_) => {
                    info!("Connected");
                    break;
                }
                Err(err) => {
                    error!("Failed to connect: {err}");
                    match err {
                        LoginError::HttpRequestFailed(err) if err.is_timeout() => {}
                        _ => break,
                    }
                }
            }
            try_count += 1;
        }
        debounce_begin = Instant::now();
    }
    Ok(())
}
