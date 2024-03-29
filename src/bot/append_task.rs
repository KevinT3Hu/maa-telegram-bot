use teloxide::{
    payloads::{AnswerCallbackQuerySetters, SendMessageSetters},
    requests::{Request, Requester},
    types::CallbackQuery,
    Bot,
};

use crate::model::TaskType;

use super::{
    append_task, get_devices_markup, get_is_single_user, get_single_device_and_user,
    get_tasks_markup, get_users_markup, BotDialog, DialogState, HandlerResult,
};

pub async fn start_append_task_dialog(bot: Bot, dialog: BotDialog) -> HandlerResult {
    // If only one user is present, no need to ask for device and user
    if get_is_single_user()? {
        let (device, user) = get_single_device_and_user()?;
        dialog
            .update(DialogState::AppendTaskToUser {
                device_id: device.id.clone(),
                user_id: user.id.clone(),
            })
            .await?;

        bot.send_message(
            dialog.chat_id(),
            format!("Select task for device {}, user {}", device.name, user.id),
        )
        .reply_markup(get_tasks_markup())
        .await?;

        return Ok(());
    }

    dialog.update(DialogState::StartAppendTask).await?;

    let sent_msg = "Select device:".to_owned();

    bot.send_message(dialog.chat_id(), sent_msg)
        .reply_markup(get_devices_markup()?)
        .await?;

    Ok(())
}

pub async fn receive_device(bot: Bot, dialog: BotDialog, q: CallbackQuery) -> HandlerResult {
    if let Some(ref device_id) = q.data {
        if !device_id.starts_with("d:") {
            bot.send_message(dialog.chat_id(), "Invalid device id")
                .send()
                .await?;
            bot.answer_callback_query(q.id).show_alert(false).await?;
            return Ok(());
        }

        let device_id = device_id.replace("d:", "");

        #[allow(clippy::unwrap_used)]
        let current_state = dialog.get().await?.unwrap();

        let next_state = match current_state {
            DialogState::StartAppendTask => DialogState::AppendTaskToDevice {
                device_id: device_id.clone(),
            },
            DialogState::StartAppendHeartBeatTask => {
                DialogState::AppendHeartBeatTaskToDevice {
                    device_id: device_id.clone(),
                }
            }
            DialogState::Idle | DialogState::AppendTaskToDevice{ .. } | DialogState::AppendTaskToUser{ .. } | DialogState::AppendHeartBeatTaskToDevice{ .. } => {
                bot.send_message(dialog.chat_id(), "Invalid state")
                    .send()
                    .await?;
                bot.answer_callback_query(q.id).show_alert(false).await?;
                return Ok(());
            }
        };

        dialog.update(next_state).await?;

        bot.answer_callback_query(q.id).show_alert(false).await?;

        bot.send_message(dialog.chat_id(), "Select user")
            .reply_markup(get_users_markup(&device_id)?)
            .await?;
    }

    Ok(())
}

pub async fn receive_user(
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

        #[allow(clippy::unwrap_used)]
        let current_state = dialog.get().await?.unwrap();

        if let DialogState::AppendHeartBeatTaskToDevice { .. } = current_state {
            dialog.exit().await?;

            append_task(&device_id, &user_id, &TaskType::HeartBeat.to_string())?;

            bot.answer_callback_query(q.id).show_alert(true).await?;
            return Ok(());
        }

        dialog
            .update(DialogState::AppendTaskToUser {
                device_id: device_id.clone(),
                user_id: user_id.clone(),
            })
            .await?;

        bot.answer_callback_query(q.id).show_alert(false).await?;

        bot.send_message(dialog.chat_id(), "Select task")
            .reply_markup(get_tasks_markup())
            .await?;
    }

    Ok(())
}

pub async fn receive_task(
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

        append_task(&device_id, &user_id, &task)?;

        bot.answer_callback_query(q.id).show_alert(true).await?;

        bot.send_message(dialog.chat_id(), "Task added").await?;
    }

    Ok(())
}
