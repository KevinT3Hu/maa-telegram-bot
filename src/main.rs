use std::{
    collections::HashMap, fs::create_dir_all, net::SocketAddr, process::exit, sync::{atomic::{AtomicBool, Ordering}, Arc, RwLock}
};

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use axum_macros::debug_handler;
use base64::{prelude::BASE64_STANDARD, Engine};
use clap::Parser;
use config::{AppCommand, DeviceInfo};
use error::AppError;
use model::{Device, GetTaskReq, GetTaskResponse, TaskStatus, TaskType};
use once_cell::sync::OnceCell;
use teloxide::{
    requests::{Request, Requester},
    types::{ChatId, InputFile},
    Bot,
};
use tokio::net::TcpListener;
use tracing_appender::rolling::daily;

use crate::{config::Config, model::User};

mod bot;
mod config;
mod model;
mod error;

static BOT_STATE: OnceCell<Arc<AppState>> = OnceCell::new();

#[derive(Debug)]
struct AppState {
    pub devices: Arc<RwLock<HashMap<String, Device>>>,
    pub all_tasks: Arc<RwLock<HashMap<String, TaskType>>>,
    pub tg_user_id: i64,
    pub bot: Bot,
    pub is_single_user: AtomicBool,
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
            all_tasks: Arc::new(RwLock::new(HashMap::new())),
            tg_user_id,
            bot,
            is_single_user: AtomicBool::new(false),
            allowed_devices,
        }
    }
}

#[tokio::main]
async fn main() {
    let config = AppCommand::parse();
    let config = Config::new(&config.config_file);

    if let Some(ref logging_dir) = config.logging_dir {
        if let Err(e) = create_dir_all(logging_dir){
            tracing::error!("Error creating logging dir: {}", e);
        };
        let file_appender = daily(logging_dir, "maa-tgbot.log");
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
        .with_state(Arc::clone(&app_state));

    if let Err(_e) = BOT_STATE.set(Arc::clone(&app_state)) {
        tracing::error!("Error setting BOT_STATE");
        exit(1);
    }

    let address = SocketAddr::from(([127, 0, 0, 1], config.port));

    let listener = TcpListener::bind(&address)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Error binding to {}: {}", address, e);
            exit(1);
        });

    tokio::spawn(async move {
        bot::setup(bot_clone).await.unwrap_or_else(|e| {
            tracing::error!("Error setting up bot: {}", e);
            exit(1);
        });
    });

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Error serving: {}", e);
        exit(1);
    }
}

fn get_task_type(app_state: &Arc<AppState>, task_id: &str) -> Result<TaskType,AppError> {
    let all_tasks = app_state.all_tasks.read()?;

    let task_type = all_tasks.get(task_id).ok_or(AppError::TaskNotFound(task_id.to_owned()))?;

    Ok(task_type.clone())
}

// Method: POST
// Content-Type: application/json
#[debug_handler]
async fn report_status(
    app_state: State<Arc<AppState>>,
    Json(req): Json<TaskStatus>,
) -> Result<StatusCode, AppError> {
    tracing::info!("Report status");

    let task_type = get_task_type(&app_state, &req.task)?;

    let notify_msg = format!("Task {} finished. Status: {}", task_type, req.status);

    app_state
        .bot
        .send_message(ChatId(app_state.tg_user_id), notify_msg)
        .send()
        .await?;

    // handle and send payload
    match task_type {
        TaskType::CaptureImage | TaskType::CaptureImageNow => {
            let payload = req.payload;
            // convert base64 image to bytes
            #[allow(clippy::unwrap_used)]
            let payload = BASE64_STANDARD.decode(payload.as_bytes()).unwrap();

            let photo = InputFile::memory(payload);

            app_state
                .bot
                .send_photo(ChatId(app_state.tg_user_id), photo)
                .send()
                .await?;
        }
        TaskType::HeartBeat => {
            let payload = req.payload;

            if payload.is_empty() {
                app_state
                    .bot
                    .send_message(ChatId(app_state.tg_user_id), "No task is running.")
                    .await?;
                return Ok(StatusCode::OK);
            }

            let response_task_type = get_task_type(&app_state, &payload)?;

            let msg = format!("Task {response_task_type} is running.\nTask id: {payload}");

            app_state
                .bot
                .send_message(ChatId(app_state.tg_user_id), msg)
                .send()
                .await?;

            return Ok(StatusCode::OK);
        }
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

// Method: POST
// Content-Type: application/json
#[debug_handler]
async fn get_task(
    app_state: State<Arc<AppState>>,
    Json(req): Json<GetTaskReq>,
) -> Result<Json<GetTaskResponse>, AppError> {
    let mut devices = app_state.devices.write()?;

    if !devices.contains_key(&req.device) {
        if let Some(ref allowed_devices) = app_state.allowed_devices {
            if allowed_devices.contains_key(&req.device) {
                let device_name = allowed_devices.get(&req.device).ok_or(AppError::DeviceNotFound(req.device.clone()))?;
                tracing::info!("New allowed device: {} ({})", req.device, device_name);
                devices.insert(
                    req.device.clone(),
                    Device::new_with_name(req.device.clone(), device_name.clone()),
                );
            } else {
                return Ok(Json(GetTaskResponse { tasks: vec![] }));
            }
        } else {
            tracing::info!("New device: {}", req.device);
            devices.insert(req.device.clone(), Device::new(&req.device));
        }
    }

    let device_length = devices.len();
    let device = devices.get_mut(&req.device).ok_or(AppError::DeviceNotFound(req.device))?;
    let users = &mut device.users;
    let user = users.entry(req.user.clone()).or_insert(User {
        id: req.user.clone(),
        tasks: vec![],
    });
    let tasks = user.tasks.clone();

    if device_length == 1 && users.len() == 1 {
        app_state
            .is_single_user
            .store(true, Ordering::Release);
    } else {
        app_state
            .is_single_user
            .store(false, Ordering::Release);
    }

    Ok(Json(GetTaskResponse { tasks }))
}
