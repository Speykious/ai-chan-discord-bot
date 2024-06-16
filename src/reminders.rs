use std::{
	collections::VecDeque,
	fs::File,
	io::{self, BufReader, BufWriter, Read, Write},
	sync::{mpsc, Arc},
	thread,
	time::SystemTime,
};

use chrono::{DateTime, TimeDelta, Timelike, Utc};
use serenity::all::{ChannelId, CreateMessage, UserId};

use crate::AiChan;

pub fn date_time_now() -> chrono::DateTime<Utc> {
	chrono::DateTime::<Utc>::from_timestamp_micros(
		SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_micros() as i64,
	)
	.unwrap()
}

pub struct Reminder {
	pub timestamp: i64,
	pub user_id: UserId,
	pub channel_id: ChannelId,
	pub message: String,
}

impl Reminder {
	fn write(&self, w: &mut impl Write) -> io::Result<()> {
		w.write_all(&self.timestamp.to_le_bytes())?;
		w.write_all(&self.user_id.get().to_le_bytes())?;
		w.write_all(&self.channel_id.get().to_le_bytes())?;
		w.write_all(&(self.message.len() as u64).to_le_bytes())?;
		w.write_all(self.message.as_bytes())
	}

	fn read(r: &mut impl Read) -> io::Result<Self> {
		let mut timestamp_bytes = [0_u8; 8];
		r.read_exact(&mut timestamp_bytes)?;
		let timestamp = i64::from_le_bytes(timestamp_bytes);

		let mut user_id_bytes = [0_u8; 8];
		r.read_exact(&mut user_id_bytes)?;
		let user_id = UserId::new(u64::from_le_bytes(user_id_bytes));

		let mut channel_id_bytes = [0_u8; 8];
		r.read_exact(&mut channel_id_bytes)?;
		let channel_id = ChannelId::new(u64::from_le_bytes(channel_id_bytes));

		let mut message_len_bytes = [0_u8; 8];
		r.read_exact(&mut message_len_bytes)?;
		let message_len = u64::from_le_bytes(message_len_bytes) as usize;

		let mut message_bytes = vec![0_u8; message_len];
		r.read_exact(message_bytes.as_mut_slice())?;
		let message = String::from_utf8(message_bytes).unwrap();

		Ok(Self {
			timestamp,
			user_id,
			channel_id,
			message,
		})
	}
}

const REMINDERS_FILE_NAME: &str = "ai-chan-reminders.bin";

pub fn load_reminders() -> io::Result<VecDeque<Reminder>> {
	let mut reminders = VecDeque::new();

	let Ok(file) = File::open(REMINDERS_FILE_NAME) else {
		return Ok(reminders);
	};

	let mut r = BufReader::new(file);

	let mut reminders_len_bytes = [0_u8; 8];
	r.read_exact(&mut reminders_len_bytes)?;
	let reminders_len = u64::from_le_bytes(reminders_len_bytes) as usize;

	for _ in 0..reminders_len {
		reminders.push_back(Reminder::read(&mut r)?);
	}

	Ok(reminders)
}

pub fn store_reminders(reminders: &VecDeque<Reminder>) -> io::Result<()> {
	let mut w = BufWriter::new(File::create(REMINDERS_FILE_NAME)?);

	w.write_all(&(reminders.len() as u64).to_le_bytes())?;
	for reminder in reminders {
		reminder.write(&mut w)?;
	}

	Ok(())
}

pub async fn process_reminders_every_second(stop_channel: mpsc::Receiver<()>, ai_chan: AiChan) {
	loop {
		if stop_channel.try_recv().is_ok() {
			break;
		}

		let now = date_time_now().with_nanosecond(0).unwrap();

		if ai_chan.http.read().unwrap().is_some() {
			loop {
				let first_timestamp = ai_chan.reminders.read().unwrap().front().map(|r| r.timestamp);
				if let Some(first_timestamp) = first_timestamp {
					let date_time = DateTime::<Utc>::from_timestamp(first_timestamp, 0).unwrap();
                    tracing::info!("there is a reminder with date {}", date_time);

					if date_time <= now {
						let reminder = ai_chan.reminders.write().unwrap().pop_front().unwrap();

						let content = format!(
							"<@{}> Here's your reminder~\n\n{}",
							reminder.user_id.get(),
							reminder.message
						);

						let http = Arc::clone(ai_chan.http.read().unwrap().as_ref().unwrap());
						let result = (reminder.channel_id)
							.send_message(&http, CreateMessage::new().content(content))
							.await;

						if let Err(e) = result {
							tracing::error!("Cannot send reminder message: {e}");
						}
					} else {
                        break;
                    }
				} else {
					break;
				}
			}
		}

		sleep_until_next_second();
	}
}

fn sleep_until_next_second() {
	let now = date_time_now();

	let prev_time = now.time().with_nanosecond(0).unwrap();
	let next_time = prev_time + TimeDelta::seconds(1);
	let next_time = now.with_time(next_time).unwrap().to_utc();

	let remaining = next_time.signed_duration_since(now).to_std().unwrap_or_default();
	thread::sleep(remaining);
}
