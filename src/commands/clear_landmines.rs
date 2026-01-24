// I would like to thank Max0r for having this in his own server and therefore implanting the brain worm in my head of "Hey that'd be funny to write as a joke PR".

use std::{
	collections::{HashMap, VecDeque},
	sync::Arc,
};

use serenity::all::{
	ChannelId, CommandInteraction, Context, CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage,
	InteractionContext, Permissions,
};
use tokio::sync::Mutex;

use crate::commands::landmine::Landmine;

pub const NAME: &str = "clear-landmines";
pub const DESCRIPTION: &str = "Remove landmines in this channel";

pub fn register() -> CreateCommand {
	CreateCommand::new(NAME)
		.default_member_permissions(Permissions::MANAGE_MESSAGES)
		.contexts(vec![InteractionContext::Guild])
		.description(DESCRIPTION)
}

pub async fn run(
	landmine_list: Arc<Mutex<HashMap<ChannelId, VecDeque<Landmine>>>>,
	ctx: &Context,
	command: &CommandInteraction,
) {
	let content = if landmine_list.lock().await.remove(&command.channel_id).is_some() {
		let channel_name = (command.channel.as_ref())
			.and_then(|c| c.name.as_deref())
			.unwrap_or("???");

		tracing::info!("Cleared landmines for channel {}", channel_name);

		"No more landmines in this channel. You are safe for now... :3c"
	} else {
		"This channel didn't have landmines in the first place!"
	};

	let builder = CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(content));
	if let Err(e) = command.create_response(&ctx.http, builder).await {
		tracing::error!("Cannot respond to slash command: {e}");
	}
}
