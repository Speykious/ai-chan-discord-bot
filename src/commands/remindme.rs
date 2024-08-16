use std::{
	collections::VecDeque,
	num::ParseIntError,
	ops::Deref,
	sync::{atomic::Ordering, Arc, RwLock},
};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Utc};
use serenity::all::{
	CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse,
	CreateInteractionResponseMessage,
};

use crate::reminders::{date_time_now, store_reminders, Reminder, NEXT_REMINDER_ID};

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

const WRONG: &str = r#"

Valid time formats include:
- a duration
  - valid suffixes: `d` `h` `m` `s`, `hr(s)` `min(s)` `sec(s)`, `day(s)` `hour(s)` `minute(s)` `second(s)`
  - duration format: `<number><suffix> <number><suffix> <number><suffix> ...` (no space between number and suffix)
  - examples: `1d 3h 10m`, `23day`, `35hrs 4min`, `727secs`
- a UTC date
  - valid formats: `YYYY-MM-DD`, `YYYY-MM-DD hh:mm`, `YYYY-MM-DD hh:mm:ss`"#;

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

	if let Some(date_time) = parse_time_delta(&now, time) {
		let ts = date_time.timestamp();
		timestamp = Some(ts);

		content = format!("Okie, will remind you <t:{ts}:R> ~");
	} else {
		match parse_date_time(&now, time) {
			Ok(date_time) => {
				let ts = date_time.timestamp();
				timestamp = Some(ts);

				content = format!("Okie, will remind you on <t:{ts}:F> ~");
			}
			Err(e) => {
				timestamp = None;

				content = match e {
					ParseDateTimeError::UnrecognizedDateFormat => {
						"I don't recognize this date format! I only know `YYYY-MM-DD`.".to_string()
					}
					ParseDateTimeError::UnrecognizedTimeFormat => {
						"I don't recognize this time format! I only know `hh:mm` and `hh:mm:ss`.".to_string()
					}
					ParseDateTimeError::ParseYear(pie) => {
						format!("Was that a number for the year? I don't get it :c\n`{pie}`")
					}
					ParseDateTimeError::ParseMonth(pie) => {
						format!("Was that a number for the month? I don't get it :c\n`{pie}`")
					}
					ParseDateTimeError::ParseDay(pie) => {
						format!("Was that a number for the day? I don't get it :c\n`{pie}`")
					}
					ParseDateTimeError::InvalidDate => "This date is invalid!".to_string(),
					ParseDateTimeError::InvalidMonth => {
						"This month is invalid! There is no more of them after December~".to_string()
					}
					ParseDateTimeError::InvalidDay => {
						"This day is invalid! There are never more than 31 days~".to_string()
					}
					ParseDateTimeError::ParseHour(pie) => {
						format!("Was that a number for the hours? I don't get it :c\n`{pie}`")
					}
					ParseDateTimeError::ParseMin(pie) => {
						format!("Was that a number for the minutes? I don't get it :c\n`{pie}`")
					}
					ParseDateTimeError::ParseSec(pie) => {
						format!("Was that a number for the seconds? I don't get it :c\n`{pie}`")
					}
					ParseDateTimeError::InvalidHour => {
						"This hour is invalid! I don't know how to count after 23, tehe :P".to_string()
					}
					ParseDateTimeError::InvalidMin => "This minute is invalid!".to_string(),
					ParseDateTimeError::InvalidSec => "This second is invalid!".to_string(),
				};

				content += WRONG;
			}
		};
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

			let id = NEXT_REMINDER_ID.load(Ordering::Relaxed);
			NEXT_REMINDER_ID.store(id + 1, Ordering::Relaxed);

			reminders.insert(
				idx,
				Reminder {
					id,
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

enum ParseDateTimeError {
	UnrecognizedDateFormat,
	UnrecognizedTimeFormat,

	ParseYear(ParseIntError),
	ParseMonth(ParseIntError),
	ParseDay(ParseIntError),

	InvalidDate,

	InvalidMonth,
	InvalidDay,

	ParseHour(ParseIntError),
	ParseMin(ParseIntError),
	ParseSec(ParseIntError),

	InvalidHour,
	InvalidMin,
	InvalidSec,
}

fn parse_date_time(now: &DateTime<Utc>, input: &str) -> Result<DateTime<Utc>, ParseDateTimeError> {
	let input = input.trim();
	let (date_parts, naive_time) = if let Some((date_part, time_part)) = input.split_once(' ') {
		let time_part = time_part.trim().split(':').collect::<Vec<_>>();

		let (hour, min, sec) = if let &[h, m, s] = time_part.as_slice() {
			(h, m, s)
		} else if let &[h, m] = time_part.as_slice() {
			(h, m, "0")
		} else {
			return Err(ParseDateTimeError::UnrecognizedTimeFormat);
		};

		let hour: u32 = hour.parse().map_err(ParseDateTimeError::ParseHour)?;
		let min: u32 = min.parse().map_err(ParseDateTimeError::ParseMin)?;
		let sec: u32 = sec.parse().map_err(ParseDateTimeError::ParseSec)?;

		if hour >= 24 {
			return Err(ParseDateTimeError::InvalidHour);
		}
		if min >= 60 {
			return Err(ParseDateTimeError::InvalidMin);
		}
		if sec >= 60 {
			return Err(ParseDateTimeError::InvalidSec);
		}

		let naive_time = NaiveTime::from_hms_opt(hour, min, sec).unwrap();

		let date_parts = date_part.trim().split('-').collect::<Vec<_>>();
		(date_parts, naive_time)
	} else {
		let naive_time = now.naive_utc().time();

		let date_parts = input.split('-').collect::<Vec<_>>();
		(date_parts, naive_time)
	};

	let [year, month, day] = date_parts.as_slice() else {
		return Err(ParseDateTimeError::UnrecognizedDateFormat);
	};

	let year: i32 = year.parse().map_err(ParseDateTimeError::ParseYear)?;
	let month: u32 = month.parse().map_err(ParseDateTimeError::ParseMonth)?;
	let day: u32 = day.parse().map_err(ParseDateTimeError::ParseDay)?;

	if month > 12 {
		return Err(ParseDateTimeError::InvalidMonth);
	}
	if day > 31 {
		return Err(ParseDateTimeError::InvalidDay);
	}

	let naive_date = NaiveDate::from_ymd_opt(year, month, day).ok_or(ParseDateTimeError::InvalidDate)?;

	Ok(NaiveDateTime::new(naive_date, naive_time).and_utc())
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
			"d" | "day" | "days" => time_delta += TimeDelta::days(n),
			"h" | "hr" | "hrs" | "hour" | "hours" => time_delta += TimeDelta::hours(n),
			"m" | "min" | "mins" | "minute" | "minutes" => time_delta += TimeDelta::minutes(n),
			"s" | "sec" | "secs" | "second" | "seconds" => time_delta += TimeDelta::seconds(n),
			_ => return None,
		}
	}

	now.checked_add_signed(time_delta)
}
