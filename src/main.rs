use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serenity::all::{ChannelId, CurrentUser, GatewayIntents, Ready};
use serenity::builder::CreateMessage;
use serenity::client::Cache;
use serenity::model::prelude::Message;
use serenity::prelude::Context;
use serenity::Client;

use tokio::time::sleep;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone, Copy)]
pub enum Oops {
	Ping,
	Reply,
}

pub struct EventHandler {
	bot: Arc<RwLock<Option<CurrentUser>>>,
	soliloquy: Arc<RwLock<Option<ChannelId>>>,
	cache: Cache,
}

impl EventHandler {
	pub fn new() -> Self {
		Self {
			bot: Arc::new(RwLock::new(None)),
			soliloquy: Arc::new(RwLock::new(None)),
			cache: Cache::new(),
		}
	}
}

impl Default for EventHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[serenity::async_trait]
impl serenity::client::EventHandler for EventHandler {
	async fn ready(&self, ctx: Context, data: Ready) {
		tracing::info!(
			"Ready! Invite link: https://discord.com/api/oauth2/authorize?client_id={}&permissions=11264&scope=bot",
			data.user.id
		);
		*self.bot.write().unwrap() = Some(data.user);

		for guild in data.guilds {
			if let Some(guild_name) = guild.id.name(&self.cache) {
				tracing::info!("Scanning guild {}...", guild_name);
			}

			let channels = match guild.id.channels(&ctx.http).await {
				Ok(channels) => channels,
				Err(e) => {
					tracing::error!("Unable to fetch channels for guild {}!", guild.id);
					tracing::error!("{}", e);
					continue;
				}
			};

			for channel in channels {
				tracing::debug!("Discovered {} ({})", channel.0, channel.1.name);
				if channel.1.name == "soliloquy" {
					tracing::info!("#soliloquy channel registered.");
					self.soliloquy.write().unwrap().replace(channel.0);
				}
			}
		}
	}

	async fn message(&self, ctx: Context, message: Message) {
		if !(self.soliloquy.read().unwrap()).is_some_and(|id| id == message.channel_id) {
			// ignore non-soliloquy messages
			return;
		}

		if message.author.id == self.bot.read().unwrap().as_ref().unwrap().id {
			// ignore own messages
			return;
		}

		if !message.mentions.is_empty() {
			oops(Oops::Ping, ctx, message).await;
			return;
		}

		if message.referenced_message.is_some()
            // do not match meta-messages
            && !(message.content.starts_with('[') && message.content.ends_with(']'))
		{
			oops(Oops::Reply, ctx, message).await;
			return;
		}
	}
}

const PER_CHANNEL_RULES: &str =
	"As per the channel rules, this channel is meant as a space where you can monologue, and interactions are thus forbidden.";

async fn oops(oops_kind: Oops, ctx: Context, message: Message) {
	if let Err(e) = message.delete(&ctx.http).await {
		tracing::error!("Could not delete message: {}", e);
	}

	let oops_msg = match oops_kind {
		Oops::Ping => "Please, do not mention people in #soliloquy!",
		Oops::Reply => "Please, do not reply to other messages in #soliloquy!",
	};

	// zero-width space nyehehehe
	let sanitized_message_content = message.content.replace('`', "\u{200B}`");

	let youshallnotpass = format!(
		"{} {}\n\n*Original message:*\n```\n{}\n```",
		oops_msg, PER_CHANNEL_RULES, &sanitized_message_content
	);

	let content = CreateMessage::new().content(&youshallnotpass);

	if message.author.dm(&ctx.http, content).await.is_ok() {
		// try DM first
		tracing::info!("Sent a DM to {}", message.author);
	} else if let Ok(response) = message.reply_ping(&ctx.http, youshallnotpass).await {
		// try in-channel
		tracing::info!("Replied to {}", message.author);

		sleep(Duration::from_secs(7)).await;
		if let Err(e) = response.delete(&ctx.http).await {
			tracing::error!("Could not delete response: {}", e);
		}
	} else {
		// give up :(
		tracing::warn!("Could not send a message to {}. I give up :(", message.author);
	}
}

#[tokio::main]
async fn main() {
	dotenvy::dotenv().expect(".env file not found");

	tracing_subscriber::registry()
		.with(tracing_subscriber::fmt::layer())
		.with(LevelFilter::INFO)
		.init();

	tracing::info!("NAMTAO #soliloquy channel cleaner init...");
	let token = env::var("TOKEN").expect("No token provided in env var TOKEN");

	tracing::info!(
		"Token hash: {}",
		token.as_bytes().iter().map(|x| *x as u32).sum::<u32>()
	);
	let mut client = Client::builder(&token, GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT)
		.event_handler(EventHandler::new())
		.await
		.expect("unable to init client");

	tracing::info!("Initialized.");
	client.start().await.unwrap();
}
