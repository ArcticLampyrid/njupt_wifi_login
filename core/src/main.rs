#![cfg_attr(windows, windows_subsystem = "windows")]

mod login;
mod network_changed;

use anyhow::Error;
use log::*;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{env, time::Duration};

use login::login;
use network_changed::NetworkChangedListener;

static CONFIG_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = env::current_exe().unwrap();
    path.pop();
    path.push("njupt_wifi.yml");
    path
});
static LOG_PATH: Lazy<PathBuf> = Lazy::new(|| {
    let mut path = env::current_exe().unwrap();
    path.pop();
    path.push("njupt_wifi.log");
    path
});

const TIMEOUT: Duration = Duration::from_secs(1);

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
    let file_log = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build(LOG_PATH.as_path())
        .unwrap();

    let log_config = log4rs::Config::builder()
        .appender(Appender::builder().build("file_log", Box::new(file_log)))
        .build(
            Root::builder()
                .appender("file_log")
                .build(LevelFilter::Trace),
        )
        .unwrap();

    let _ = log4rs::init_config(log_config).unwrap();

    let credential = read_config().unwrap_or_else(|error| {
        error!("Failed to read config: {}", error);
        panic!("{}", error)
    });

    let listener = NetworkChangedListener::new()?;
    let mut rx = listener.listen()?;
    info!("Network connectivity hint changed notification registered");

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(TIMEOUT)
        .connect_timeout(TIMEOUT)
        .build()?;
    while let Some(()) = rx.recv().await {
        info!("Start to login");
        match login(&client, &credential).await {
            Ok(_) => info!("Connected"),
            Err(err) => error!("Failed to connect: {}", err),
        };
    }
    Ok(())
}
