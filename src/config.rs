use clap::Parser;
use serde::Deserialize;

#[derive(Deserialize, clap::Parser, Clone, Debug)]
pub struct Config {
    #[serde(skip)]
    pub config_file: Option<String>,

    pub port: Option<u16>,

    #[arg(long)]
    pub telegram_bot_token: Option<String>,

    #[arg(long)]
    pub telegram_user_id: Option<String>,

    #[arg(skip)]
    pub devices: Option<Vec<DeviceInfo>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

macro_rules! get_attr {
    ($parsed:expr,$file:expr,$param:ident) => {
        $parsed.$param.unwrap_or_else(|| {
            let msg = std::stringify!($param).to_owned() + " not specified";
            $file.clone().expect(&msg).$param.expect(&msg)
        })
    };
}

impl Config {
    pub fn parse_config() -> Self {
        let parsed = Config::parse();

        let file_config = parsed.config_file.map(|path| {
            let str = std::fs::read_to_string(path).expect("Config file is invalid.");
            let parsed_file: Config = serde_json::from_str(&str).expect("Config file invalid");
            parsed_file
        });

        let port = get_attr!(parsed, file_config, port);

        let telegram_bot_token = get_attr!(parsed, file_config, telegram_bot_token);

        let telegram_user_name = get_attr!(parsed, file_config, telegram_user_id);

        let devices = file_config.and_then(|config| config.devices);

        Self {
            config_file: None,
            port: Some(port),
            telegram_bot_token: Some(telegram_bot_token),
            telegram_user_id: Some(telegram_user_name),
            devices,
        }
    }
}
