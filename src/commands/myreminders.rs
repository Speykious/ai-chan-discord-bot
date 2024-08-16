use std::{
	collections::VecDeque,
	fmt::Write as _,
	sync::{Arc, RwLock},
};

use serenity::all::{
	CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse,
	CreateInteractionResponseMessage,
};

use crate::reminders::Reminder;

pub const NAME: &str = "myreminders";
pub const DESCRIPTION: &str = "I'll list all your reminders~ ♡";

pub fn register() -> CreateCommand {
	CreateCommand::new(NAME)
		.add_option(CreateCommandOption::new(
			CommandOptionType::Integer,
			"id",
			"ID of the reminder you want to see (all if not specified)",
		))
		.description(DESCRIPTION)
}

pub async fn run(reminders: Arc<RwLock<VecDeque<Reminder>>>, ctx: &Context, command: &CommandInteraction) {
	let mut rem_id = None;

	for option in &command.data.options {
		match option.name.as_str() {
			"id" => rem_id = Some(option.value.as_i64().unwrap()),
			s => tracing::error!("Invalid option {s:?}"),
		}
	}

	let content = if let Some(rem_id) = rem_id {
		let reminders = reminders.read().unwrap();

		let rem = (reminders.iter()).find(|rem| rem.user_id == command.user.id && rem.id == rem_id);

		if let Some(rem) = rem {
			format!(
				"Here's your reminder~\n`{}` <t:{}:F> in <#{}>\n\n{}",
				rem.id, rem.timestamp, rem.channel_id, rem.message
			)
		} else {
			String::from("No such reminder :(")
		}
	} else {
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
