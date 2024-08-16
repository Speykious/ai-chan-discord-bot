use std::{
	collections::{HashSet, VecDeque},
	fmt::Write as _,
	ops::Deref,
	sync::{Arc, RwLock},
};

use serenity::all::{
	CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse,
	CreateInteractionResponseMessage,
};

use crate::reminders::{store_reminders, Reminder};

pub const NAME: &str = "myreminders";
pub const DESCRIPTION: &str = "I'll list all your reminders~ ♡";

pub fn register() -> CreateCommand {
	CreateCommand::new(NAME)
		.add_option(CreateCommandOption::new(
			CommandOptionType::Integer,
			"id",
			"ID of the reminder you want to see (all if not specified)",
		))
		.add_option(CreateCommandOption::new(
			CommandOptionType::Boolean,
			"delete",
			"Delete the specified reminder, or all of them (Careful, no confirm button!)",
		))
		.description(DESCRIPTION)
}

pub async fn run(reminders: Arc<RwLock<VecDeque<Reminder>>>, ctx: &Context, command: &CommandInteraction) {
	let mut rem_id = None;
	let mut delet = false;

	for option in &command.data.options {
		match option.name.as_str() {
			"id" => rem_id = Some(option.value.as_i64().unwrap()),
			"delete" => delet = option.value.as_bool().unwrap_or_default(),
			s => tracing::error!("Invalid option {s:?}"),
		}
	}

	let mut rems_to_delet = HashSet::new();

	let content = {
		let reminders = reminders.read().unwrap();

		let mut content = if let Some(rem_id) = rem_id {
			let rem = (reminders.iter())
				.find(|rem| rem.user_id == command.user.id && rem.id == rem_id)
				.cloned();

			if let Some(rem) = rem {
				if delet {
					rems_to_delet.insert(rem.id);
				}

				format!(
					"Here's your reminder~\n`{}` <t:{}:F> in <#{}>\n\n{}",
					rem.id, rem.timestamp, rem.channel_id, rem.message
				)
			} else {
				String::from("No such reminder :(")
			}
		} else {
			if delet {
				rems_to_delet.extend(
					(reminders.iter())
						.filter(|rem| rem.user_id == command.user.id)
						.map(|rem| rem.id),
				);
			}

			let reminders = (reminders.iter().take(41))
				.filter(|rem| rem.user_id == command.user.id)
				.collect::<Vec<_>>();

			let mut content = if reminders.is_empty() {
				String::from("You have no reminders! Sorry~")
			} else if reminders.len() <= 40 {
				String::from("Here are all your reminders~\n")
			} else {
				String::from("Wow, you have more than 40 reminders! Here are your oldest ones...\n")
			};

			for rem in reminders.iter().take(40) {
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

		if delet {
			content += &match rems_to_delet.len() {
				1 => "\n\n1 reminder deleted!".to_string(),
				l => format!("\n\n{l} reminders deleted!"),
			};
		}

		content
	};

	if delet {
		let mut reminders = reminders.write().unwrap();

		let new_reminders = (reminders.iter())
			.filter(|rem| !rems_to_delet.contains(&rem.id))
			.cloned()
			.collect::<VecDeque<_>>();

		*reminders = new_reminders;

		store_reminders(reminders.deref()).unwrap();
	}

	let response_message = CreateInteractionResponseMessage::new().ephemeral(true).content(content);
	let builder = CreateInteractionResponse::Message(response_message);
	if let Err(e) = command.create_response(&ctx.http, builder).await {
		tracing::error!("Cannot respond to slash command: {e}");
	}
}
