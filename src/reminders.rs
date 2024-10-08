use std::{
	collections::VecDeque,
	fs::File,
	io::{self, BufReader, BufWriter, Read, Write},
	ops::Deref,
	sync::{
		atomic::{AtomicI64, Ordering},
		Arc, RwLock,
	},
	time::SystemTime,
};

use chrono::{DateTime, TimeDelta, Timelike, Utc};
use serenity::all::{ChannelId, Context, CreateMessage, UserId};

pub fn date_time_now() -> chrono::DateTime<Utc> {
	chrono::DateTime::<Utc>::from_timestamp_micros(
		SystemTime::now()
			.duration_since(SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_micros() as i64,
	)
	.unwrap()
}

pub static NEXT_REMINDER_ID: AtomicI64 = AtomicI64::new(0);

#[derive(Clone)]
pub struct Reminder {
	pub id: i64,
	pub timestamp: i64,
	pub user_id: UserId,
	pub channel_id: ChannelId,
	pub message: String,
}

impl Reminder {
	fn write(&self, w: &mut impl Write) -> io::Result<()> {
		w.write_all(&self.id.to_le_bytes())?;
		w.write_all(&self.timestamp.to_le_bytes())?;
		w.write_all(&self.user_id.get().to_le_bytes())?;
		w.write_all(&self.channel_id.get().to_le_bytes())?;
		w.write_all(&(self.message.len() as u64).to_le_bytes())?;
		w.write_all(self.message.as_bytes())
	}

	fn read(r: &mut impl Read) -> io::Result<Self> {
		let mut id_bytes = [0_u8; 8];
		r.read_exact(&mut id_bytes)?;
		let id = i64::from_le_bytes(id_bytes);

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
			id,
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
	let reminders_len = u64::from_le_bytes(reminders_len_bytes);

	for _ in 0..reminders_len {
		reminders.push_back(Reminder::read(&mut r)?);
	}

	let next_reminder_id = reminders.iter().map(|rem| rem.id).max().unwrap_or_default() + 1;
	NEXT_REMINDER_ID.store(next_reminder_id, Ordering::Relaxed);

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

pub async fn process_reminders_every_second(reminders: Arc<RwLock<VecDeque<Reminder>>>, ctx: Context) {
	loop {
		let now = date_time_now().with_nanosecond(0).unwrap();

		loop {
			let first_timestamp = reminders.read().unwrap().front().map(|r| r.timestamp);
			if let Some(first_timestamp) = first_timestamp {
				let date_time = DateTime::<Utc>::from_timestamp(first_timestamp, 0).unwrap();

				if date_time <= now {
					let reminder = reminders.write().unwrap().pop_front().unwrap();

					let content = format!(
						"<@{}> Here's your reminder~\n\n{}",
						reminder.user_id.get(),
						reminder.message
					);

					let result = (reminder.channel_id)
						.send_message(&ctx.http, CreateMessage::new().content(content))
						.await;

					if let Err(e) = result {
						tracing::error!("Cannot send reminder message: {e}");
					}

					store_reminders(reminders.read().unwrap().deref()).unwrap();
				} else {
					break;
				}
			} else {
				break;
			}
		}

		sleep_until_next_second().await;
	}
}

async fn sleep_until_next_second() {
	let now = date_time_now();

	let prev_time = now.time().with_nanosecond(0).unwrap();
	let next_time = prev_time + TimeDelta::seconds(1);
	let next_time = now.with_time(next_time).unwrap().to_utc();

	let remaining = next_time.signed_duration_since(now).to_std().unwrap_or_default();
	tokio::time::sleep(remaining).await;
}
