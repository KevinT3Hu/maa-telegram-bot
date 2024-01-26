use std::fs::read_to_string;

use serde::Deserialize;

#[derive(clap::Parser)]
pub struct AppCommand {
    pub config_file: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub telegram_bot_token: String,
    pub telegram_user_id: i64,
    pub logging_dir: Option<String>, // will be created if not exists
    pub devices: Option<Vec<DeviceInfo>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

impl Config {

    #[allow(clippy::expect_used)]
    pub fn new(file: &str) -> Self {
        let config_file = read_to_string(file).expect("Unable to read config file");

        serde_json::from_str(&config_file).expect("Unable to parse config file")
    }
}
