use std::error::{self, Error};
use core::sync::atomic::Ordering::Acquire;
use dptree::case;
use teloxide::{
    dispatching::{
        dialogue::{self, Dialogue, InMemStorage},
        Dispatcher, UpdateFilterExt, UpdateHandler,
    },
    requests::Requester,
    types::{
        BotCommand, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, Update,
    },
    utils::command::BotCommands,
    Bot,
};
use tracing::info;

use crate::{
    config::DeviceInfo, error::AppError, model::{Task, TaskType, User}, BOT_STATE
};

mod append_task;
mod get_current_task;
mod screenshot_all;

type HandlerResult = Result<(), Box<dyn error::Error + Send + Sync>>;

pub async fn setup(bot: Bot) -> Result<(),AppError> {
    bot.set_my_commands(vec![
        BotCommand::new("appendtask", "Append task"),
        BotCommand::new("screenshotall", "Take screenshot for all bound devices"),
        BotCommand::new("getcurrenttask", "Get current running task"),
    ])
    .await?;

    Dispatcher::builder(bot.clone(), schema())
        .dependencies(dptree::deps![InMemStorage::<DialogState>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

    Ok(())
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    AppendTask,
    ScreenshotAll,
    GetCurrentTask,
}

#[derive(Clone)]
pub enum DialogState {
    Idle,
    StartAppendTask,
    AppendTaskToDevice { device_id: String },
    AppendTaskToUser { device_id: String, user_id: String },
    StartAppendHeartBeatTask,
    AppendHeartBeatTaskToDevice { device_id: String },
}

impl Default for DialogState {
    fn default() -> Self {
        Self::Idle
    }
}

type BotDialog = Dialogue<DialogState, InMemStorage<DialogState>>;

fn get_users(device_id: &str) -> Result<Vec<String>,AppError> {
    let app_state = BOT_STATE.get().ok_or(AppError::StateNotSet)?;

    app_state.devices.read()?.get(device_id).map(|d| {
        d.users
            .values()
            .map(|u| u.id.clone())
            .collect::<Vec<String>>()
    }).ok_or(AppError::DeviceNotFound(device_id.to_owned()))
}

fn get_devices(use_name: bool) -> Result<Vec<String>,AppError> {
    let app_state = BOT_STATE.get().ok_or(AppError::StateNotSet)?;

    let devices = app_state
        .devices
        .read()?
        .values()
        .map(|d| {
            if use_name {
                d.name.clone()
            } else {
                d.id.clone()
            }
        })
        .collect();

    Ok(devices)
}

fn get_is_single_user() -> Result<bool,AppError> {
    let app_state = BOT_STATE.get().ok_or(AppError::StateNotSet)?;

    Ok(app_state
            .is_single_user
            .load(Acquire))
}

fn get_single_device_and_user() -> Result<(DeviceInfo, User),AppError> {
    let app_state = BOT_STATE.get().ok_or(AppError::StateNotSet)?;

    info!("App state: {:?}", app_state);

    let devices = app_state.devices.read()?;

    #[allow(clippy::unwrap_used)]
    let device = devices.values().next().unwrap();

    #[allow(clippy::unwrap_used)]
    let user = device.users.values().next().unwrap().clone();

    Ok((device.clone().into(), user))
}

// TODO: should this be a method of AppState?
fn append_task(device_id: &str, user_id: &str, task: &str) -> Result<(),AppError> {
    let app_state = BOT_STATE.get().ok_or(AppError::StateNotSet)?;

    let mut devices = app_state.devices.write()?;

    let mut all_states = app_state.all_tasks.write()?;

    let user = devices
        .get_mut(device_id)
        .ok_or(AppError::DeviceNotFound(device_id.to_owned()))?
        .users
        .get_mut(user_id)
        .ok_or(AppError::UserNotFound(user_id.to_owned()))?;

    let task = Task::from_str(task);
    let task_id = task.id.clone();
    let task_type = task.task_type.clone();
    user.tasks.push(task);
    all_states.insert(task_id, task_type.clone());

    // append an extra CaptureImage task if the task itself is not
    if !matches!(task_type, TaskType::CaptureImage) {
        let cap_task = Task::capture_image_task();
        let cap_task_id = cap_task.id.clone();
        user.tasks.push(cap_task);
        all_states.insert(cap_task_id, TaskType::CaptureImage);
    }

    Ok(())
}

fn get_permitted_user_id() -> Result<i64,AppError> {
    let app_state = BOT_STATE.get().ok_or(AppError::StateNotSet)?;

    Ok(app_state.tg_user_id)
}

fn get_tasks_markup() -> InlineKeyboardMarkup {
    let tasks = TaskType::get_all();
    let tasks = tasks
        .iter()
        .map(|t| {
            let callback_text = format!("t:{t}");
            InlineKeyboardButton::new(t, InlineKeyboardButtonKind::CallbackData(callback_text))
        })
        .map(|t| vec![t]);

    InlineKeyboardMarkup::new(tasks)
}

fn get_devices_markup() -> Result<InlineKeyboardMarkup,AppError> {
    let devices = get_devices(true);

    let devices = devices?
        .into_iter()
        .map(|d| {
            let callback_text = format!("d:{d}");
            InlineKeyboardButton::new(d, InlineKeyboardButtonKind::CallbackData(callback_text))
        })
        .map(|d| vec![d]);

    Ok(InlineKeyboardMarkup::new(devices))
}

fn get_users_markup(device_id: &str) -> Result<InlineKeyboardMarkup,AppError> {
    let users = get_users(device_id);

    let users = users?
        .into_iter()
        .map(|u| {
            let callback_text = format!("u:{u}");
            InlineKeyboardButton::new(u, InlineKeyboardButtonKind::CallbackData(callback_text))
        })
        .map(|u| vec![u]);

    Ok(InlineKeyboardMarkup::new(users))
}

fn schema() -> UpdateHandler<Box<dyn Error + Send + Sync + 'static>> {
    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![Command::AppendTask].endpoint(append_task::start_append_task_dialog))
        .branch(case![Command::ScreenshotAll].endpoint(screenshot_all::take_screenshot_all))
        .branch(
            case![Command::GetCurrentTask]
                .endpoint(get_current_task::start_get_current_task_dialog),
        );

    let msg_handler = Update::filter_message().branch(command_handler);

    let callback_handler = Update::filter_callback_query()
        .branch(case![DialogState::StartAppendTask].endpoint(append_task::receive_device))
        .branch(
            case![DialogState::StartAppendHeartBeatTask].endpoint(append_task::receive_device),
        )
        .branch(
            case![DialogState::AppendTaskToDevice { device_id }]
                .endpoint(append_task::receive_user),
        )
        .branch(
            case![DialogState::AppendHeartBeatTaskToDevice { device_id }]
                .endpoint(append_task::receive_user),
        )
        .branch(
            case![DialogState::AppendTaskToUser { device_id, user_id }]
                .endpoint(append_task::receive_task),
        );

    dialogue::enter::<Update, InMemStorage<DialogState>, DialogState, _>()
            .chain(dptree::filter(|dialog: BotDialog| {
                let permitted_user_id = get_permitted_user_id().unwrap_or(-1);

                let chat_id = dialog.chat_id();

                chat_id.is_user() && chat_id.0 == permitted_user_id
            }))
            .branch(msg_handler)
            .branch(callback_handler)
}
