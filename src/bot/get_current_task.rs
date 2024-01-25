use teloxide::{payloads::SendMessageSetters, requests::Requester, Bot};

use crate::model::TaskType;

use super::{
    append_task, get_devices_markup, get_is_single_user, get_single_device_and_user, BotDialog,
    BotDialogState, HandlerResult,
};

pub async fn start_get_current_task_dialog(bot: Bot, dialog: BotDialog) -> HandlerResult {
    // If only one user is present, no need to ask for device and user
    if get_is_single_user() {
        if let Some((device, user)) = get_single_device_and_user() {
            dialog.exit().await?;

            append_task(&device.id, &user.id, &TaskType::HeartBeat.to_string());

            return Ok(());
        }
    }

    dialog
        .update(BotDialogState::StartAppendHeartBeatTask)
        .await?;

    let sent_msg = "Select device:".to_string();

    bot.send_message(dialog.chat_id(), sent_msg)
        .reply_markup(get_devices_markup())
        .await?;

    Ok(())
}
