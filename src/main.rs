use std::{
    collections::HashMap,
    net::SocketAddr,
    ops::Deref,
    sync::{Arc, RwLock},
};

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use axum_macros::debug_handler;
use base64::{prelude::BASE64_STANDARD, Engine};
use clap::Parser;
use config::{AppCommand, DeviceInfo};
use model::{Device, GetTaskReq, GetTaskResponse, TaskStatus, TaskType};
use once_cell::sync::OnceCell;
use teloxide::{
    requests::{Request, Requester},
    types::{ChatId, InputFile},
    Bot,
};
use tokio::net::TcpListener;


use crate::{config::Config, model::User};

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
    let config = AppCommand::parse();
    let config = Config::new(&config.config_file);

    if let Some(logging_dir) = &config.logging_dir {
        std::fs::create_dir_all(logging_dir).expect("Unable to create logging dir");
        let file_appender = tracing_appender::rolling::daily(logging_dir, "maa-tgbot.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        tracing_subscriber::fmt()
            .pretty()
            .with_ansi(false)
            .with_writer(non_blocking)
            .init();
        tracing::info!("Logging to: {}", logging_dir);
    } else {
        tracing_subscriber::fmt().with_ansi(false).init();
    }

    let bot = teloxide::Bot::new(config.telegram_bot_token);

    let bot_clone = bot.clone();

    let allowed_devices = config.devices;

    let app_state = Arc::new(AppState::new(
        config.telegram_user_id,
        bot,
        &allowed_devices,
    ));

    let app = Router::new()
        .route("/report", post(report_status))
        .route("/get", post(get_task))
        .with_state(app_state.clone());

    BOT_STATE.set(app_state.clone()).unwrap();

    let address = SocketAddr::from(([127, 0, 0, 1], config.port));

    let listener = TcpListener::bind(&address)
        .await
        .expect("Error creating Tcp listener");

    tokio::spawn(async move {
        bot::setup_bot(bot_clone).await;
    });

    println!("Staring server...");

    axum::serve(listener, app).await.unwrap();
}

fn get_task_type(
    app_state: Arc<AppState>,
    task_id: &str,
    device_id: &str,
    user_id: &str,
) -> TaskType {
    let devices = app_state.devices.read().unwrap();

    let device = devices.get(device_id).unwrap();
    let user_tasks = device.users.get(user_id).unwrap().tasks.clone();

    let task = user_tasks.iter().find(|t| t.id == task_id).unwrap();

    task.task_type.clone()
}

// Method: POST
// Content-Type: application/json
#[debug_handler]
async fn report_status(
    app_state: State<Arc<AppState>>,
    Json(req): Json<TaskStatus>,
) -> Result<StatusCode, ()> {
    tracing::info!("Report status");

    let task_type = get_task_type(app_state.deref().clone(), &req.task, &req.device, &req.user);

    let msg = format!("Task {} finished. Status: {}", task_type, req.status);

    app_state
        .bot
        .send_message(ChatId(app_state.tg_user_id), msg)
        .send()
        .await
        .unwrap();

    // handle and send payload
    match task_type {
        TaskType::CaptureImage => {
            let payload = req.payload;
            // convert base64 image to bytes
            let payload = BASE64_STANDARD.decode(payload.as_bytes()).unwrap();

            let photo = InputFile::memory(payload);

            app_state
                .bot
                .send_photo(ChatId(app_state.tg_user_id), photo)
                .send()
                .await
                .unwrap();
        },
        TaskType::LinkStartCombat
        | TaskType::LinkStartBase
        | TaskType::LinkStartWakeUp
        | TaskType::LinkStartMall
        | TaskType::LinkStartMission
        | TaskType::LinkStartAutoRoguelike
        | TaskType::LinkStartReclamationAlgorithm
        | TaskType::LinkStartRecruiting => {}
    }

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

    Ok(Json(GetTaskResponse {
        tasks: user.tasks.clone(),
    }))
}
