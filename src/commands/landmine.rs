// I would like to thank Max0r for having this in his own server and therefore implanting the brain worm in my head of "Hey that'd be funny to write as a joke PR".

use std::{
	collections::{HashMap, VecDeque},
	sync::Arc,
};

use chrono::TimeDelta;
use rand::random_range;
use serenity::all::{
	CacheHttp, ChannelId, CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
	CreateInteractionResponse, CreateInteractionResponseMessage, EditMember, InteractionContext, Message, Permissions,
	Timestamp,
};
use tokio::sync::Mutex;

pub const NAME: &str = "landmine";
pub const DESCRIPTION: &str = "Spawn landmines (random user timeouts) in this channel 😱💥";

pub fn register() -> CreateCommand {
	CreateCommand::new(NAME)
		.default_member_permissions(Permissions::MANAGE_MESSAGES)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::Integer,
				"count",
				"Number of landmines to spawn in this channel",
			)
			.min_int_value(1)
			.max_int_value(20),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::Integer,
				"messages",
				"Max message count before the landmine detonates",
			)
			.min_int_value(5)
			.max_int_value(40),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::Integer,
				"minutes",
				"Max duration of the timeout in minutes",
			)
			.min_int_value(2)
			.max_int_value(10),
		)
		.contexts(vec![InteractionContext::Guild])
		.description(DESCRIPTION)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Landmine {
	/// Number of messages before the landmine detonates
	delay: u8,
	/// Max duration of the timeout once the landmine detonates
	minutes: u8,
}

pub async fn run(
	landmine_list: Arc<Mutex<HashMap<ChannelId, VecDeque<Landmine>>>>,
	ctx: &Context,
	command: &CommandInteraction,
) {
	let mut landmines = 5;
	let mut messages = 10;
	let mut minutes = 2;

	for option in &command.data.options {
		match option.name.as_str() {
			"count" => landmines = option.value.as_i64().unwrap_or(5).clamp(1, 20) as u8,
			"messages" => messages = option.value.as_i64().unwrap_or(2).clamp(5, 40) as u8,
			"minutes" => minutes = option.value.as_i64().unwrap_or(2).clamp(2, 10) as u8,
			s => tracing::error!("Invalid option {s:?}"),
		}
	}

	let mut local_landmines = VecDeque::new();
	for _ in 0..landmines {
		let delay = rand::random_range(5..=messages);
		let minutes = random_range(2..=minutes);
		local_landmines.push_back(Landmine { delay, minutes });
	}

	tracing::info!(
		"Spawned {} landmines (max delay = {} messages, max timeout = {} minutes); next one is in {} messages",
		landmines,
		messages,
		minutes,
		local_landmines[0].delay
	);

	(landmine_list.lock().await).insert(command.channel_id, local_landmines);
	let content = format!("There are now {} landmines in this channel. Users beware... :3c ♡\n-# max delay = **{}** messages, max timeout = **{}** minutes", landmines, messages, minutes);

	let builder = CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(content));
	if let Err(e) = command.create_response(&ctx.http, builder).await {
		tracing::error!("Cannot respond to slash command: {e}");
	}
}

pub async fn handle_message(
	channel_landmines: Arc<Mutex<HashMap<ChannelId, VecDeque<Landmine>>>>,
	ctx: &Context,
	message: &Message,
) {
	if message.author.bot {
		return;
	}

	let mut channel_landmines = channel_landmines.lock().await;
	let Some(landmines) = channel_landmines.get_mut(&message.channel_id) else {
		return;
	};

	let Some(first_landmine) = landmines.front_mut() else {
		return;
	};

	if first_landmine.delay == 0 {
		let first_landmine = landmines.pop_front().unwrap();

		tracing::info!(
			"User {} stepped on a landmine and will be timed out for {} minutes!",
			message.author.id,
			first_landmine.minutes,
		);

		if let Some(guild) = message.guild_id {
			let time = match Timestamp::now().checked_add_signed(TimeDelta::minutes(first_landmine.minutes as i64)) {
				Some(dt) => dt,
				None => {
					tracing::error!(
						"We somehow failed to create a timestamp for now + {} minutes...",
						first_landmine.minutes
					);
					return;
				}
			};

			let builder = EditMember::new().disable_communication_until_datetime(time.into());
			match guild.edit_member(ctx.http(), message.author.id, builder).await {
				Ok(_) => {
					let content = format!(":boom: <@{}> stepped on a landmine and has been timed out for **{} minutes!**\n-# {} Landmines remain~", message.author.id, first_landmine.minutes, landmines.len());
					if let Err(why) = message.channel_id.say(&ctx.http, content).await {
						tracing::error!("Error sending message: {why:?}");
					}
				}
				Err(why) => tracing::error!("Failed to time out user: {why:?}"),
			}
		}

		if landmines.is_empty() {
			let message_free_of_mines = "**No more landmines remain!** You are safe, for now... :3c";

			if let Err(why) = message.channel_id.say(&ctx.http, message_free_of_mines).await {
				tracing::error!("Error sending message: {why:?}");
			}
		}
	} else {
		first_landmine.delay -= 1;
	}
}
