use teloxide::{
    requests::{Request, Requester},
    Bot,
};

use crate::{model::TaskType, BOT_STATE};

use super::{append_task, BotDialog, HandlerResult};

fn append_screenshot_to_all() {
    let app_state = BOT_STATE.get().unwrap().clone();

    let devices = app_state.devices.read().unwrap();

    for device in devices.values() {
        for user in device.users.values() {
            append_task(&device.id, &user.id, &TaskType::CaptureImageNow.to_string());
        }
    }
}

#[allow(clippy::module_name_repetitions)]
pub async fn take_screenshot_all(bot: Bot, dialog: BotDialog) -> HandlerResult {
    append_screenshot_to_all();

    bot.send_message(dialog.chat_id(), "Tasks sent.")
        .send()
        .await?;
    Ok(())
}
