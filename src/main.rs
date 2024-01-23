use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use axum_macros::debug_handler;
use config::{Config, DeviceInfo};
use model::{Device, GetTaskReq, GetTaskResponse, TaskStatus};
use once_cell::sync::OnceCell;
use teloxide::{
    requests::{Request, Requester},
    types::ChatId,
    Bot,
};
use tokio::net::TcpListener;

use crate::model::User;

mod bot;
mod config;
mod model;

static BOT_STATE: OnceCell<Arc<AppState>> = OnceCell::new();

#[derive(Clone, Debug)]
struct AppState {
    pub devices: Arc<RwLock<HashMap<String, Device>>>,
    pub tg_user_id: i64,
    pub bot: Bot,
    pub allowed_devices: Option<HashMap<String, String>>,
}

impl AppState {
    pub fn new(tg_user_id: i64, bot: Bot, allowed_devices: &Option<Vec<DeviceInfo>>) -> Self {
        let allowed_devices = allowed_devices.clone().map(|devices| {
            devices
                .iter()
                .map(|device| (device.id.clone(), device.name.clone()))
                .collect()
        });

        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            tg_user_id,
            bot,
            allowed_devices,
        }
    }
}

#[tokio::main]
async fn main() {
    let config = Config::parse_config();

    let bot = teloxide::Bot::new(config.telegram_bot_token.unwrap());

    let tg_user_id: i64 = config
        .telegram_user_id
        .unwrap()
        .parse()
        .expect("Invalid user id");

    let bot_clone = bot.clone();

    let allowed_devices = config.devices;

    let app_state = Arc::new(AppState::new(tg_user_id, bot, &allowed_devices));

    let app = Router::new()
        .route("/report", post(report_status))
        .route("/get", post(get_task))
        .with_state(app_state.clone());

    BOT_STATE.set(app_state.clone()).unwrap();

    let address = SocketAddr::from(([127, 0, 0, 1], config.port.unwrap()));

    let listener = TcpListener::bind(&address)
        .await
        .expect("Error creating Tcp listener");

    tokio::spawn(async move {
        bot::setup_bot(bot_clone).await;
    });

    println!("Staring server...");

    axum::serve(listener, app).await.unwrap();
}

// Method: POST
// Content-Type: application/json
#[debug_handler]
async fn report_status(
    app_state: State<Arc<AppState>>,
    Json(req): Json<TaskStatus>,
) -> Result<StatusCode, ()> {
    println!("Recevied report");

    let msg = format!(
        "User: {}, Device: {}, Task: {}, Status: {}, Payload: {}",
        req.user, req.device, req.task, req.status, req.payload
    );

    app_state
        .bot
        .send_message(ChatId(app_state.tg_user_id), msg)
        .send()
        .await
        .unwrap();

    Ok(StatusCode::OK)
}

async fn get_task(
    app_state: State<Arc<AppState>>,
    Json(req): Json<GetTaskReq>,
) -> Result<Json<GetTaskResponse>, ()> {
    let mut devices = app_state.devices.write().unwrap();

    if !devices.contains_key(&req.device) {
        if let Some(allowed_devices) = &app_state.allowed_devices {
            if !allowed_devices.contains_key(&req.device) {
                return Ok(Json(GetTaskResponse { tasks: vec![] }));
            }
        }
        devices.insert(req.device.clone(), Device::new(req.device.clone()));
    }

    let device = devices.get_mut(&req.device).unwrap();
    let user = device.users.entry(req.user.clone()).or_insert(User {
        id: req.user.clone(),
        tasks: vec![],
    });

    let str = serde_json::to_string(&GetTaskResponse {
        tasks: user.tasks.clone(),
    })
    .unwrap();
    println!("Get task: {}", str);

    Ok(Json(GetTaskResponse {
        tasks: user.tasks.clone(),
    }))
}
