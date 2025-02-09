use std::time::Duration;

use chrono::{DateTime, Utc};
use serenity::all::{
	CommandDataOption, CommandDataOptionValue, CommandInteraction, CommandOptionType, Context, CreateCommand,
	CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, EditMember, InteractionContext,
};

pub const NAME: &str = "selfmute";
pub const DESCRIPTION: &str = "Mute yourself for a specified amount of minutes :x";

pub fn register() -> CreateCommand {
	CreateCommand::new(NAME)
		.description(DESCRIPTION)
		.contexts(vec![InteractionContext::Guild])
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::Number,
				"minutes",
				"Duration of time you want to be muted for (5 minutes if unspecified)",
			),
		)
}

pub async fn run(ctx: &Context, mut command: CommandInteraction) {
	let minutes = match &*command.data.options {
		[CommandDataOption {
			name,
			value: CommandDataOptionValue::Number(m),
			..
		}] if name == "minutes" => *m,

		[] => 5.0,

		options => {
			tracing::error!("Unexpected options: {options:?}");
			return;
		}
	};

	let content: String = 'content: {
		if minutes.is_sign_negative() {
			break 'content "You can't mute yourself a negative amount of time?!".into();
		}

		if minutes == 0.0 {
			break 'content "Muting yourself for zero seconds is a little bit silly :3c".into();
		}

		let Some(member) = &mut command.member else {
			break 'content "Command is only usable in a guild!".into();
		};

		let until: DateTime<Utc> = Utc::now() + Duration::from_secs_f64(minutes * 60.);

		let mute_until = EditMember::new().disable_communication_until_datetime(until.into());

		match member.edit(ctx, mute_until).await {
			Ok(()) => format!("Muted until <t:{0}:f> (<t:{0}:R>). Have a nice rest~", until.timestamp()),
			Err(e) => {
				tracing::error!("Cannot mute member: {e}");
				"Unfortunately couldn't mute you :(".to_string()
			}
		}
	};

	let response_message = CreateInteractionResponseMessage::new().ephemeral(true).content(content);
	let builder = CreateInteractionResponse::Message(response_message);
	if let Err(e) = command.create_response(&ctx.http, builder).await {
		tracing::error!("Cannot respond to slash command: {e}");
	}
}
