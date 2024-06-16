use std::sync::RwLock;
use std::time::Duration;

use serenity::all::{ChannelId, CurrentUser};
use serenity::builder::CreateMessage;
use serenity::model::prelude::Message;
use serenity::prelude::Context;

use tokio::time::sleep;

static SOLILOQUY: ChannelId = ChannelId::new(1137703122408575077);

pub async fn handle_message(bot: &RwLock<Option<CurrentUser>>, ctx: Context, message: Message) {
	if message.channel_id != SOLILOQUY {
		// ignore non-soliloquy messages
		return;
	}

	if message.author.id == bot.read().unwrap().as_ref().unwrap().id {
		// ignore own messages
		return;
	}

	if !message.mentions.is_empty() {
		oops(OOPS_PING, ctx, message).await;
		return;
	}

	if message.referenced_message.is_some()
            // do not match meta-messages
            && !(message.content.starts_with('[') && message.content.ends_with(']'))
	{
		oops(OOPS_REPLY, ctx, message).await;
	}
}

const OOPS_PING: &str = "Please, do not mention people in #soliloquy!";
const OOPS_REPLY: &str = "Please, do not reply to other messages in #soliloquy!";
const PER_CHANNEL_RULES: &str =
	"As per the channel rules, this channel is meant as a space where you can monologue, and interactions are thus forbidden.";

async fn oops(oops_msg: &str, ctx: Context, message: Message) {
	if let Err(e) = message.delete(&ctx.http).await {
		tracing::error!("Could not delete message: {}", e);
	}

	// zero-width space nyehehehe
	let sanitized_message_content = message.content.replace('`', "\u{200B}`");

	let you_shall_not_pass = format!(
		"{} {}\n\n*Original message~*\n```\n{}\n```",
		oops_msg, PER_CHANNEL_RULES, &sanitized_message_content
	);

	let content = CreateMessage::new().content(&you_shall_not_pass);

	if message.author.dm(&ctx.http, content).await.is_ok() {
		// try DM first
		tracing::info!("Sent a DM to {}", message.author);
	} else if let Ok(response) = message.reply_ping(&ctx.http, you_shall_not_pass).await {
		// try in-channel
		tracing::info!("Replied to {}", message.author);

		sleep(Duration::from_secs(7)).await;
		if let Err(e) = response.delete(&ctx.http).await {
			tracing::error!("Could not delete response: {}", e);
		}
	} else {
		// give up :(
		tracing::warn!("Could not send a message to {}. I give up :c", message.author);
	}
}
