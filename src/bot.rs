use dptree::case;
use teloxide::{
    dispatching::{
        dialogue::{self, Dialogue, InMemStorage},
        Dispatcher, UpdateFilterExt, UpdateHandler,
    },
    payloads::{AnswerCallbackQuerySetters, SendMessageSetters},
    requests::{Request, Requester},
    types::{
        BotCommand, CallbackQuery, InlineKeyboardButton, InlineKeyboardButtonKind,
        InlineKeyboardMarkup, Update,
    },
    utils::command::BotCommands,
    Bot,
};
use tracing::info;

use crate::{
    config::DeviceInfo, model::{Task, TaskType, User}, BOT_STATE
};

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

pub async fn setup_bot(bot: Bot) {
    bot.set_my_commands(vec![
        BotCommand::new("appendtask", "Append task"),
        BotCommand::new("setdevicename", "Set device display name"),
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
    SetDeviceName,
}

#[derive(Clone)]
pub enum BotDialogState {
    Idle,
    StartSetNameForDevice,
    SetNameDevice { device_id: String },
    StartAppendTask,
    AppendTaskToDevice { device_id: String },
    AppendTaskToUser { device_id: String, user_id: String },
}

impl Default for BotDialogState {
    fn default() -> Self {
        Self::Idle
    }
}

type BotDialog = Dialogue<BotDialogState, InMemStorage<BotDialogState>>;

fn get_users(device_id: String) -> Vec<String> {
    let app_state = BOT_STATE.get().unwrap().clone();

    let users = app_state.devices.read().unwrap().get(&device_id).map(|d| {
        d.users
            .values()
            .map(|u| u.id.clone())
            .collect::<Vec<String>>()
    });

    users.unwrap()
}

fn get_devices(use_name:bool) -> Vec<String> {
    let app_state = BOT_STATE.get().unwrap().clone();

    let devices = app_state
        .devices
        .read()
        .unwrap()
        .values()
        .map(|d|
            if use_name {
                d.name.clone()
            } else {
                d.id.clone()
            }
        )
        .collect();

    devices
}

fn get_is_single_user() -> bool {
    let app_state = BOT_STATE.get().unwrap().clone();

    app_state
        .is_single_user
        .load(std::sync::atomic::Ordering::Acquire)
}

fn get_single_device_and_user() -> Option<(DeviceInfo,User)> {
    let app_state = BOT_STATE.get().unwrap().clone();

    info!("App state: {:?}", app_state);

    let devices = app_state.devices.read().unwrap();

    let device = devices.values().next().unwrap();

    let user = device.users.values().next().unwrap().clone();

    Some((device.clone().into(), user))
}

async fn start_append_task_dialog(bot: Bot, dialog: BotDialog) -> HandlerResult {
    // If only one user is present, no need to ask for device and user
    if get_is_single_user() {
        if let Some((device, user)) = get_single_device_and_user() {

            dialog
                .update(BotDialogState::AppendTaskToUser {
                    device_id: device.id.clone(),
                    user_id: user.id.clone(),
                })
                .await?;

            let tasks = TaskType::get_all();
            let tasks = tasks
                .iter()
                .map(|t| {
                    let callback_text = format!("t:{}", t);
                    InlineKeyboardButton::new(
                        t,
                        InlineKeyboardButtonKind::CallbackData(callback_text),
                    )
                })
                .map(|t| vec![t]);

            let btn_markup = InlineKeyboardMarkup::new(tasks);

            bot.send_message(
                dialog.chat_id(),
                format!("Select task for device {}, user {}", device.name, user.id),
            )
            .reply_markup(btn_markup)
            .await?;

            return Ok(());
        }
    }

    dialog.update(BotDialogState::StartAppendTask).await?;

    let devices = get_devices(true);

    let sent_msg = "Select device:".to_string();

    let devices = devices
        .iter()
        .map(|d| {
            let callback_text = format!("d:{}", d);
            InlineKeyboardButton::new(d, InlineKeyboardButtonKind::CallbackData(callback_text))
        })
        .map(|d| vec![d]);
    let btn_markup = InlineKeyboardMarkup::new(devices);

    bot.send_message(dialog.chat_id(), sent_msg)
        .reply_markup(btn_markup)
        .await?;

    Ok(())
}

async fn receive_device(bot: Bot, dialog: BotDialog, q: CallbackQuery) -> HandlerResult {
    if let Some(ref device_id) = q.data {
        if !device_id.starts_with("d:") {
            bot.send_message(dialog.chat_id(), "Invalid device id")
                .send()
                .await?;
            bot.answer_callback_query(q.id).show_alert(false).await?;
            return Ok(());
        }

        let device_id = device_id.replace("d:", "");

        dialog
            .update(BotDialogState::AppendTaskToDevice {
                device_id: device_id.clone(),
            })
            .await?;

        let users = get_users(device_id.clone());

        let users = users
            .iter()
            .map(|u| {
                let callback_text = format!("u:{}", u);
                InlineKeyboardButton::new(u, InlineKeyboardButtonKind::CallbackData(callback_text))
            })
            .map(|u| vec![u]);

        let btn_markup = InlineKeyboardMarkup::new(users);

        bot.answer_callback_query(q.id).show_alert(false).await?;

        bot.send_message(dialog.chat_id(), "Select user")
            .reply_markup(btn_markup)
            .await?;
    }

    Ok(())
}

async fn receive_user(
    bot: Bot,
    dialog: BotDialog,
    device_id: String,
    q: CallbackQuery,
) -> HandlerResult {
    if let Some(ref user_id) = q.data {
        if !user_id.starts_with("u:") {
            bot.send_message(dialog.chat_id(), "Invalid user id")
                .send()
                .await?;
            bot.answer_callback_query(q.id).show_alert(false).await?;
            return Ok(());
        }

        let user_id = user_id.replace("u:", "");

        dialog
            .update(BotDialogState::AppendTaskToUser {
                device_id: device_id.clone(),
                user_id: user_id.clone(),
            })
            .await?;

        let tasks = TaskType::get_all();
        let tasks = tasks
            .iter()
            .map(|t| {
                let callback_text = format!("t:{}", t);
                InlineKeyboardButton::new(t, InlineKeyboardButtonKind::CallbackData(callback_text))
            })
            .map(|t| vec![t]);

        let btn_markup = InlineKeyboardMarkup::new(tasks);

        bot.answer_callback_query(q.id).show_alert(false).await?;

        bot.send_message(dialog.chat_id(), "Select task")
            .reply_markup(btn_markup)
            .await?;
    }

    Ok(())
}

fn append_task(device_id: &str, user_id: &str, task: &str) {
    let app_state = BOT_STATE.get().unwrap().clone();

    let mut devices = app_state.devices.write().unwrap();

    let user = devices
        .get_mut(device_id)
        .unwrap()
        .users
        .get_mut(user_id)
        .unwrap();

    let task = Task::from_str(task);
    let task_type = task.task_type.clone();
    user.tasks.push(task);

    // append an extra CaptureImage task if the task itself is not
    if !matches!(task_type, TaskType::CaptureImage) {
        let task = Task::new_capture_image_task();
        user.tasks.push(task);
    }
}

async fn receive_task(
    bot: Bot,
    dialog: BotDialog,
    (device_id, user_id): (String, String),
    q: CallbackQuery,
) -> HandlerResult {
    if let Some(ref task) = q.data {
        if !task.starts_with("t:") {
            bot.send_message(dialog.chat_id(), "Invalid task")
                .send()
                .await?;
            bot.answer_callback_query(q.id).show_alert(false).await?;
            return Ok(());
        }

        let task = task.replace("t:", "");

        dialog.exit().await?;

        append_task(&device_id, &user_id, &task);

        bot.answer_callback_query(q.id).show_alert(true).await?;

        bot.send_message(dialog.chat_id(), "Task added").await?;
    }

    Ok(())
}

fn get_permitted_user_id() -> i64 {
    let app_state = BOT_STATE.get().unwrap().clone();

    app_state.tg_user_id
}

async fn set_name_for_device(bot: Bot, dialog: BotDialog) -> HandlerResult {
    dialog.update(BotDialogState::StartSetNameForDevice).await?;

    let devices = get_devices(false);
    let devices_markup = devices
        .iter()
        .map(|d| {
            let callback_text = format!("d:{}", d);
            InlineKeyboardButton::new(d, InlineKeyboardButtonKind::CallbackData(callback_text))
        })
        .map(|d| vec![d]);

    bot.send_message(dialog.chat_id(), "Set name for device\nNote that this setting only affects this instance. If you want to have a name persistent to all instances, you need to set that in the bot config file.")
        .reply_markup(InlineKeyboardMarkup::new(devices_markup))
        .await?;

    Ok(())
}

async fn set_name_receive_device(bot: Bot, dialog: BotDialog, q: CallbackQuery) -> HandlerResult {
    if let Some(ref device_id) = q.data {
        if !device_id.starts_with("d:") {
            bot.send_message(dialog.chat_id(), "Invalid device id")
                .send()
                .await?;
            bot.answer_callback_query(q.id).show_alert(false).await?;
            return Ok(());
        }

        let device_id = device_id.replace("d:", "");

        dialog
            .update(BotDialogState::SetNameDevice {
                device_id: device_id.clone(),
            })
            .await?;

        bot.answer_callback_query(q.id).show_alert(false).await?;

        bot.send_message(dialog.chat_id(), "Set name for the selected device")
            .await?;

        return Ok(());
    }

    Ok(())
}

fn set_device_name(device_id: &str, name: &str) {
    let app_state = BOT_STATE.get().unwrap().clone();

    let mut devices = app_state.devices.write().unwrap();

    let device = devices.get_mut(device_id).unwrap();

    device.name = name.to_owned();
    info!("Device {} name set to {}", device_id, name);
}

async fn receive_device_name(
    bot: Bot,
    dialog: BotDialog,
    device_id: String,
    msg: String,
) -> HandlerResult {
    set_device_name(&device_id, &msg);

    info!("msg: {}",msg);

    bot.send_message(dialog.chat_id(), "Device name set")
        .await?;

    Ok(())
}

fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![Command::AppendTask].endpoint(start_append_task_dialog))
        .branch(case![Command::SetDeviceName].endpoint(set_name_for_device));

    let msg_handler = Update::filter_message()
        .branch(command_handler)
        .branch(case![BotDialogState::SetNameDevice { device_id }].endpoint(receive_device_name));

    let callback_handler = Update::filter_callback_query()
        .branch(case![BotDialogState::StartAppendTask].endpoint(receive_device))
        .branch(case![BotDialogState::AppendTaskToDevice { device_id }].endpoint(receive_user))
        .branch(
            case![BotDialogState::AppendTaskToUser { device_id, user_id }].endpoint(receive_task),
        )
        .branch(case![BotDialogState::StartSetNameForDevice].endpoint(set_name_receive_device));

    dialogue::enter::<Update, InMemStorage<BotDialogState>, BotDialogState, _>()
        .chain(dptree::filter(|dialog: BotDialog| {
            let permitted_user_id = get_permitted_user_id();

            let chat_id = dialog.chat_id();

            chat_id.is_user() && chat_id.0 == permitted_user_id
        }))
        .branch(msg_handler)
        .branch(callback_handler)
}
