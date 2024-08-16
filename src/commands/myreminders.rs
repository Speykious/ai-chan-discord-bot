use std::{
	collections::VecDeque,
	fmt::Write as _,
	sync::{Arc, RwLock},
};

use serenity::all::{
	CommandInteraction, Context, CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage,
};

use crate::reminders::Reminder;

pub const NAME: &str = "myreminders";
pub const DESCRIPTION: &str = "I'll list all your reminders~ ♡";

pub fn register() -> CreateCommand {
	CreateCommand::new(NAME).description(DESCRIPTION)
}

pub async fn run(reminders: Arc<RwLock<VecDeque<Reminder>>>, ctx: &Context, command: &CommandInteraction) {
	let content = {
		let reminders = reminders.read().unwrap();

		let mut content = if reminders.len() <= 100 {
			String::from("Here are all your reminders~\n")
		} else {
			String::from("Wow, you have more than 100 reminders! Here are your oldest ones...\n")
		};

		for rem in reminders.iter().take(100).filter(|rem| rem.user_id == command.user.id) {
			let message = if rem.message.len() <= 80 {
				rem.message.clone()
			} else {
				format!("{}...", &rem.message[..37])
			};

			// replace backticks with grave accent to avoid breaking the display
			// make sure no newlines or weird whitespace
			let message = message.replace('`', "ˋ").replace(['\n', '\t'], " ");

			write!(
				&mut content,
				"\n- `{}` <t:{}:F> in <#{}> `{}`",
				rem.id, rem.timestamp, rem.channel_id, message
			)
			.unwrap();
		}

		content
	};

	let response_message = CreateInteractionResponseMessage::new().ephemeral(true).content(content);
	let builder = CreateInteractionResponse::Message(response_message);
	if let Err(e) = command.create_response(&ctx.http, builder).await {
		tracing::error!("Cannot respond to slash command: {e}");
	}
}
