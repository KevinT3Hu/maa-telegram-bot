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
    config::DeviceInfo,
    model::{Task, TaskType, User},
    BOT_STATE,
};

mod append_task;
mod get_current_task;
mod screenshot_all;

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub async fn setup_bot(bot: Bot) {
    bot.set_my_commands(vec![
        BotCommand::new("appendtask", "Append task"),
        BotCommand::new("screenshotall", "Take screenshot for all bound devices"),
        BotCommand::new("getcurrenttask", "Get current running task"),
    ])
    .await
    .unwrap();

    Dispatcher::builder(bot.clone(), schema())
        .dependencies(dptree::deps![InMemStorage::<BotDialogState>::new()])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum Command {
    AppendTask,
    ScreenshotAll,
    GetCurrentTask,
}

#[derive(Clone)]
pub enum BotDialogState {
    Idle,
    StartAppendTask,
    AppendTaskToDevice { device_id: String },
    AppendTaskToUser { device_id: String, user_id: String },
    StartAppendHeartBeatTask,
    AppendHeartBeatTaskToDevice { device_id: String },
}

impl Default for BotDialogState {
    fn default() -> Self {
        Self::Idle
    }
}

type BotDialog = Dialogue<BotDialogState, InMemStorage<BotDialogState>>;

fn get_users(device_id: &str) -> Vec<String> {
    let app_state = BOT_STATE.get().unwrap().clone();

    let users = app_state.devices.read().unwrap().get(device_id).map(|d| {
        d.users
            .values()
            .map(|u| u.id.clone())
            .collect::<Vec<String>>()
    });

    users.unwrap()
}

fn get_devices(use_name: bool) -> Vec<String> {
    let app_state = BOT_STATE.get().unwrap().clone();

    let devices = app_state
        .devices
        .read()
        .unwrap()
        .values()
        .map(|d| {
            if use_name {
                d.name.clone()
            } else {
                d.id.clone()
            }
        })
        .collect();

    devices
}

fn get_is_single_user() -> bool {
    let app_state = BOT_STATE.get().unwrap().clone();

    app_state
        .is_single_user
        .load(std::sync::atomic::Ordering::Acquire)
}

fn get_single_device_and_user() -> Option<(DeviceInfo, User)> {
    let app_state = BOT_STATE.get().unwrap().clone();

    info!("App state: {:?}", app_state);

    let devices = app_state.devices.read().unwrap();

    let device = devices.values().next().unwrap();

    let user = device.users.values().next().unwrap().clone();

    Some((device.clone().into(), user))
}

// TODO: should this be a method of AppState?
fn append_task(device_id: &str, user_id: &str, task: &str) {
    let app_state = BOT_STATE.get().unwrap().clone();

    let mut devices = app_state.devices.write().unwrap();

    let mut all_states = app_state.all_tasks.write().unwrap();

    let user = devices
        .get_mut(device_id)
        .unwrap()
        .users
        .get_mut(user_id)
        .unwrap();

    let task = Task::from_str(task);
    let task_id = task.id.clone();
    let task_type = task.task_type.clone();
    user.tasks.push(task);
    all_states.insert(task_id, task_type.clone());

    // append an extra CaptureImage task if the task itself is not
    if !matches!(task_type, TaskType::CaptureImage) {
        let task = Task::capture_image_task();
        let task_id = task.id.clone();
        user.tasks.push(task);
        all_states.insert(task_id, TaskType::CaptureImage);
    }
}

fn get_permitted_user_id() -> i64 {
    let app_state = BOT_STATE.get().unwrap().clone();

    app_state.tg_user_id
}

fn get_tasks_markup() -> InlineKeyboardMarkup {
    let tasks = TaskType::get_all();
    let tasks = tasks
        .iter()
        .map(|t| {
            let callback_text = format!("t:{}", t);
            InlineKeyboardButton::new(t, InlineKeyboardButtonKind::CallbackData(callback_text))
        })
        .map(|t| vec![t]);

    InlineKeyboardMarkup::new(tasks)
}

fn get_devices_markup() -> InlineKeyboardMarkup {
    let devices = get_devices(true);

    let devices = devices
        .iter()
        .map(|d| {
            let callback_text = format!("d:{}", d);
            InlineKeyboardButton::new(d, InlineKeyboardButtonKind::CallbackData(callback_text))
        })
        .map(|d| vec![d]);

    InlineKeyboardMarkup::new(devices)
}

fn get_users_markup(device_id: &str) -> InlineKeyboardMarkup {
    let users = get_users(device_id);

    let users = users
        .iter()
        .map(|u| {
            let callback_text = format!("u:{}", u);
            InlineKeyboardButton::new(u, InlineKeyboardButtonKind::CallbackData(callback_text))
        })
        .map(|u| vec![u]);

    InlineKeyboardMarkup::new(users)
}

fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![Command::AppendTask].endpoint(append_task::start_append_task_dialog))
        .branch(case![Command::ScreenshotAll].endpoint(screenshot_all::take_screenshot_all))
        .branch(
            case![Command::GetCurrentTask]
                .endpoint(get_current_task::start_get_current_task_dialog),
        );

    let msg_handler = Update::filter_message().branch(command_handler);

    let callback_handler = Update::filter_callback_query()
        .branch(case![BotDialogState::StartAppendTask].endpoint(append_task::receive_device))
        .branch(
            case![BotDialogState::StartAppendHeartBeatTask].endpoint(append_task::receive_device),
        )
        .branch(
            case![BotDialogState::AppendTaskToDevice { device_id }]
                .endpoint(append_task::receive_user),
        )
        .branch(
            case![BotDialogState::AppendHeartBeatTaskToDevice { device_id }]
                .endpoint(append_task::receive_user),
        )
        .branch(
            case![BotDialogState::AppendTaskToUser { device_id, user_id }]
                .endpoint(append_task::receive_task),
        );

    dialogue::enter::<Update, InMemStorage<BotDialogState>, BotDialogState, _>()
        .chain(dptree::filter(|dialog: BotDialog| {
            let permitted_user_id = get_permitted_user_id();

            let chat_id = dialog.chat_id();

            chat_id.is_user() && chat_id.0 == permitted_user_id
        }))
        .branch(msg_handler)
        .branch(callback_handler)
}
