use std::{
	collections::VecDeque,
	ops::Deref,
	sync::{Arc, RwLock},
};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Utc};
use serenity::all::{
	CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse,
	CreateInteractionResponseMessage,
};

use crate::reminders::{date_time_now, store_reminders, Reminder};

pub const NAME: &str = "remindme";
pub const DESCRIPTION: &str = "I'll remind you whatever you want later~ ♡";

pub fn register() -> CreateCommand {
	CreateCommand::new(NAME)
		.description(DESCRIPTION)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"time",
				"Duration like 1d, 3h 10m, 5s, or specific date (UTC) like 2027-06-10 12:23:00",
			)
			.required(true),
		)
		.add_option(
			CreateCommandOption::new(CommandOptionType::String, "message", "Content of the reminder").required(true),
		)
}

pub async fn run(reminders: Arc<RwLock<VecDeque<Reminder>>>, ctx: &Context, command: &CommandInteraction) {
	let now = date_time_now();

	let mut time = None;
	let mut message = None;

	for option in &command.data.options {
		match option.name.as_str() {
			"time" => time = Some(option.value.as_str().unwrap()),
			"message" => message = Some(option.value.as_str().unwrap()),
			s => tracing::error!("Invalid option {s:?}"),
		}
	}

	let time = time.unwrap();
	let message = message.unwrap().to_string();

	let timestamp;
	let mut content;
	if let Some(date_time) = parse_date_time(&now, time) {
		let ts = date_time.timestamp();
		timestamp = Some(ts);

		content = format!("Okie, will remind you on <t:{ts}:F> ~");
	} else if let Some(date_time) = parse_time_delta(&now, time) {
		let ts = date_time.timestamp();
		timestamp = Some(ts);

		content = format!("Okie, will remind you <t:{ts}:R> ~");
	} else {
		timestamp = None;
		content = "Sorry, I couldn't understand the time you sent :c".to_string();
	};

	'remind_store: {
		if let Some(timestamp) = timestamp {
			if timestamp <= now.timestamp() {
				content = "Sweetie, I don't have a time machine! :c".to_string();
				break 'remind_store;
			}

			let user_id = command.user.id;
			let channel_id = command.channel_id;

			let mut reminders = reminders.write().unwrap();

			let idx = match reminders.binary_search_by(|r| r.timestamp.cmp(&timestamp)) {
				Ok(idx) => idx,
				Err(idx) => idx,
			};

			reminders.insert(
				idx,
				Reminder {
					timestamp,
					user_id,
					channel_id,
					message,
				},
			);

			store_reminders(reminders.deref()).unwrap();
		}
	}

	let builder = CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(content));
	if let Err(e) = command.create_response(&ctx.http, builder).await {
		tracing::error!("Cannot respond to slash command: {e}");
	}
}

fn parse_date_time(now: &DateTime<Utc>, input: &str) -> Option<DateTime<Utc>> {
	let input = input.trim();
	let (date_parts, naive_time) = if let Some((date_part, time_part)) = input.split_once(' ') {
		let time_part = time_part.trim().split(':').collect::<Vec<_>>();
		let [hour, min, sec] = time_part.as_slice() else {
			return None;
		};

		let hour: u32 = hour.parse().ok()?;
		let min: u32 = min.parse().ok()?;
		let sec: u32 = sec.parse().ok()?;

		let naive_time = NaiveTime::from_hms_opt(hour, min, sec)?;

		let date_parts = date_part.trim().split('-').collect::<Vec<_>>();
		(date_parts, naive_time)
	} else {
		let naive_time = now.naive_utc().time();

		let date_parts = input.split('-').collect::<Vec<_>>();
		(date_parts, naive_time)
	};

	let [year, month, day] = date_parts.as_slice() else {
		return None;
	};

	let year: i32 = year.parse().ok()?;
	let month: u32 = month.parse().ok()?;
	let day: u32 = day.parse().ok()?;

	let naive_date = NaiveDate::from_ymd_opt(year, month, day)?;

	Some(NaiveDateTime::new(naive_date, naive_time).and_utc())
}

fn parse_time_delta(now: &DateTime<Utc>, input: &str) -> Option<DateTime<Utc>> {
	let mut time_delta = chrono::TimeDelta::zero();

	for input_part in input.trim().split(' ') {
		let input_part = input_part.trim();
		if input_part.is_empty() {
			continue;
		}

		let digits = input_part.chars().take_while(char::is_ascii_digit).collect::<String>();
		let n_len = digits.len();
		let n: i64 = digits.parse().ok()?;

		match input_part[n_len..].trim() {
			"d" => time_delta += TimeDelta::days(n),
			"h" => time_delta += TimeDelta::hours(n),
			"m" => time_delta += TimeDelta::minutes(n),
			"s" => time_delta += TimeDelta::seconds(n),
			_ => return None,
		}
	}

	now.checked_add_signed(time_delta)
}
