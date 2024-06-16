use std::collections::VecDeque;
use std::env;
use std::sync::{mpsc, Arc, RwLock};

use reminders::{load_reminders, Reminder};
use serenity::all::{
	Command, CreateInteractionResponse, CreateInteractionResponseMessage, CurrentUser, EventHandler, GatewayIntents,
	Http, Interaction, Ready,
};
use serenity::model::prelude::Message;
use serenity::prelude::Context;
use serenity::Client;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
mod reminders;
mod soliloquy;

#[derive(Clone)]
pub struct AiChan {
	bot: Arc<RwLock<Option<CurrentUser>>>,
	reminders: Arc<RwLock<VecDeque<Reminder>>>,

	// This is the worst type I've ever seen.
	// Why am I using a NESTED Arc??
	// This is insane.
	http: Arc<RwLock<Option<Arc<Http>>>>,
}

impl AiChan {
	pub fn new(reminders: VecDeque<Reminder>) -> Self {
		Self {
			bot: Arc::new(RwLock::new(None)),
			reminders: Arc::new(RwLock::new(reminders)),
			http: Arc::new(RwLock::new(None)),
		}
	}
}

#[serenity::async_trait]
impl EventHandler for AiChan {
	async fn ready(&self, ctx: Context, data: Ready) {
		tracing::info!(
			"Ready! Invite link: https://discord.com/api/oauth2/authorize?client_id={}&permissions=11264&scope=bot",
			data.user.id
		);
		*self.bot.write().unwrap() = Some(data.user);
		*self.http.write().unwrap() = Some(Arc::clone(&ctx.http));

		match Command::set_global_commands(&ctx.http, vec![commands::remindme::register()]).await {
			Ok(_) => tracing::info!("Created global slash commands {:?}", [commands::remindme::NAME]),
			Err(e) => tracing::error!(
				"Could not create global slash command {:?}: {e}",
				commands::remindme::NAME
			),
		};
	}

	async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
		if let Interaction::Command(command) = interaction {
			println!(
				"Received command interaction {:?} from {}",
				&command.data.name, &command.user.name
			);

			match command.data.name.as_str() {
				commands::remindme::NAME => {
					commands::remindme::run(Arc::clone(&self.reminders), &ctx, &command).await;
				}
				name => {
					let builder = CreateInteractionResponse::Message(
						CreateInteractionResponseMessage::new()
							.content(format!("Sorry, I don't have any `{name}` command :c")),
					);
					if let Err(e) = command.create_response(&ctx.http, builder).await {
						tracing::error!("Cannot respond to slash command: {e}");
					}
				}
			};
		}
	}

	async fn message(&self, ctx: Context, message: Message) {
		soliloquy::handle_message(self.bot.as_ref(), ctx, message).await;
	}
}

#[tokio::main]
async fn main() {
	dotenvy::dotenv().expect(".env file not found");

	tracing_subscriber::registry()
		.with(tracing_subscriber::fmt::layer())
		.with(LevelFilter::INFO)
		.init();

	tracing::info!("AI-chan is booting up...");
	let token = env::var("TOKEN").expect("No token provided in env var TOKEN");

	tracing::info!("Loading reminders...");
	let reminders = load_reminders().expect("Could not load reminders");

	tracing::info!("Loading Discord bot client...");
	let ai_chan = AiChan::new(reminders);

	use GatewayIntents as G;
	let mut client = Client::builder(&token, G::GUILD_MESSAGES | G::MESSAGE_CONTENT)
		.event_handler(ai_chan.clone())
		.await
		.expect("Cannot initialize AI-chan! D:");

	tracing::info!(">> Hi~ ♡");

	let (stop_tx, stop_rx) = mpsc::channel();
	let handle = tokio::spawn(async {
		reminders::process_reminders_every_second(stop_rx, ai_chan).await;
	});

	client.start().await.unwrap();

	stop_tx.send(()).unwrap();
	handle.await.unwrap();
}
