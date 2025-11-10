use serenity::all::{
	CacheHttp, ChannelType, CommandInteraction, CommandType, Context, CreateCommand, CreateInteractionResponse,
	CreateInteractionResponseMessage, InteractionContext, ResolvedTarget,
};

pub const NAME: &str = "Pin/unpin thread or post message";

pub fn register() -> CreateCommand {
	CreateCommand::new(NAME)
		.kind(CommandType::Message)
		.contexts(vec![InteractionContext::Guild])
}

pub async fn run(ctx: &Context, command: CommandInteraction) {
	let thread_channel = command
		.channel
		.as_ref()
		.filter(|c| matches!(c.kind, ChannelType::PublicThread | ChannelType::PrivateThread));
	let Some(channel) = thread_channel else {
		send_ephemeral_response(
			&command,
			&ctx.http,
			"This command only works in threads or posts!",
			true,
		)
		.await;
		return;
	};

	let full_channel = match ctx.http.get_channel(channel.id).await {
		Ok(c) => c,
		Err(e) => {
			tracing::error!("Could not fetch channel {}: {e}", channel.id.get());
			send_ephemeral_response(&command, &ctx.http, "Could not get this channel info", false).await;
			return;
		}
	};

	let Some(full_channel) = full_channel.guild() else {
		tracing::error!("Guild message command called from a non-guild channel");
		return;
	};

	let Some(owner) = full_channel.owner_id else {
		tracing::error!("Command called on thread channel without an owner");
		return;
	};

	if command.user.id != owner {
		send_ephemeral_response(
			&command,
			&ctx.http,
			"Only the thread or post owner can pin messages using this command!",
			true,
		)
		.await;
		return;
	}

	let Some(ResolvedTarget::Message(message)) = command.data.target() else {
		tracing::error!("Message command called without a message");
		return;
	};

	if message.pinned {
		let res = ctx
			.http
			.unpin_message(
				channel.id,
				message.id,
				Some("Unpinning message on thread/post author request"),
			)
			.await;
		match res {
			Ok(()) => {
				send_ephemeral_response(&command, &ctx.http, "Message unpinned", false).await;
			}
			Err(e) => {
				tracing::error!("Could not unpin message {}/{}: {e}", channel.id.get(), message.id.get());
				send_ephemeral_response(&command, &ctx.http, "Could not unpin the message", false).await;
			}
		}
	} else {
		let res = ctx
			.http
			.pin_message(
				channel.id,
				message.id,
				Some("Unpinning message on thread/post author request"),
			)
			.await;
		match res {
			Ok(()) => {
				send_ephemeral_response(&command, &ctx.http, "Message pinned", false).await;
			}
			Err(e) => {
				tracing::error!("Could not pin message {}/{}: {e}", channel.id.get(), message.id.get());
				send_ephemeral_response(&command, &ctx.http, "Could not pin the message", false).await;
			}
		}
	}
}

async fn send_ephemeral_response(
	command: &CommandInteraction,
	http: impl CacheHttp,
	content: impl Into<String>,
	log_error: bool,
) {
	let response_message = CreateInteractionResponseMessage::new().content(content).ephemeral(true);

	let builder = CreateInteractionResponse::Message(response_message);

	if let Err(e) = command.create_response(http, builder).await {
		if log_error {
			tracing::error!("Cannot respond to message command: {e}");
		}
	}
}
