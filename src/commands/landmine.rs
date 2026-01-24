// I would like to thank Max0r for having this in his own server and therefore implanting the brain worm in my head of "Hey that'd be funny to write as a joke PR".

use std::{collections::{HashMap, VecDeque}, sync::{Arc}};

use chrono::TimeDelta;
use rand::random_range;
use serenity::{all::{CacheHttp, ChannelId, CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, EditMember, Message, Timestamp}, model::guild};
use tokio::sync::Mutex;

pub const NAME: &str = "landmine";
pub const DESCRIPTION: &str = "Spawn a specified amount of landmines (random user timeouts) in this channel.";

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME)
        .add_option(CreateCommandOption::new(
            CommandOptionType::Integer,
            "landmines",
            "Number of landmines to spawn in this channel"
        ))
        .description(DESCRIPTION)
}

pub async fn run(landmine_list: Arc<Mutex<HashMap<ChannelId, VecDeque<u16>>>>, ctx: &Context, command: &CommandInteraction) {
    let mut content: String = "WTF?!".to_string();

    for option in &command.data.options {
    
        match option.name.as_str() {
            "landmines" => {
                if let Some(landmines) = option.value.as_i64() {
                    let mut local_landmine_list = VecDeque::new();
                    for _ in 0..landmines {
                        // Cap this to 300 messages, just to be sensible
                        let random = rand::random_range(5..10);
                        local_landmine_list.push_back(random);
                    }
                    tracing::info!("Spawned {} landmines, next one is in {} messages", &local_landmine_list.len(), &local_landmine_list[0]);
                    landmine_list.lock().await.insert(command.channel_id, local_landmine_list);
                    content = format!("There are now {} landmines in this channel. users beware...", landmines)
                }
            }
            s => tracing::error!("Invalid option {s:?}"),
        }
    }

    let builder = CreateInteractionResponse::Message(CreateInteractionResponseMessage::new().content(content));
    if let Err(e) = command.create_response(&ctx.http, builder).await {
        tracing::error!("Cannot respond to slash command: {e}");
    }
}

pub async fn handle_message(channel_landmines: Arc<Mutex<HashMap<ChannelId, VecDeque<u16>>>>, ctx: &Context, message: &Message) {
    if let Some(landmines) = channel_landmines.lock().await.get_mut(&message.channel_id) {
        if landmines.is_empty() {
            return;
        }
        if landmines[0] == 0 {
            landmines.pop_front();
            let timeout_time = random_range(2..10);
            tracing::info!("user {} stepped on a landmine and will be timed out for {} minutes", message.author.id, timeout_time);
            if let Some(guild) = message.guild_id {
                let time = match Timestamp::now().checked_add_signed(TimeDelta::minutes(10)) {
                    Some(dt) => dt,
                    None => {
                        tracing::error!("Aw fuck, we failed to create the timestamp");
                        return;
                    }
                    ,
                };
                let builder = EditMember::new().disable_communication_until_datetime(time.into());
                match guild.edit_member(ctx.http(), message.author.id, builder).await {
                    Ok(_) => {
                        tracing::info!("Time out user {} until {}", message.author.id, time);
                        if let Some(guild) = message.guild_id {
                            let name = match message.author.nick_in(&ctx.http, guild).await {
                                Some(nick) => nick,
                                None => message.author.name.clone(),
                            };
                            let content = format!("**OOOPS!! {} stepped on a landmine and has been timed out for 10 minutes!**. {} Landmines remain", name, landmines.len());
                            if let Err(why) = message.channel_id.say(&ctx.http, content).await {
                                tracing::error!("Error sending message: {why:?}");
                            }
                        }
                    },
                    Err(why) => {
                        tracing::error!("Failed to time out user: {why:?}");
                    }
                }
            }

            if landmines.is_empty() {
                let message_free_of_mines = "**NO MORE LANDMINES REMAIN**. You are safe, for now...";

                if let Err(why) = message.channel_id.say(&ctx.http, message_free_of_mines).await {
                    tracing::error!("Error sending message: {why:?}");
                }
            }
        } else {
            landmines[0] -= 1;
        }
    }
}