// I would like to thank Max0r for having this in his own server and therefore implanting the brain worm in my head of "Hey that'd be funny to write as a joke PR".

use std::{collections::{HashMap, VecDeque}, sync::{Arc, Mutex}};

use serenity::all::{ChannelId, CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption, CreateInteractionResponse, CreateInteractionResponseMessage, Message};

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
                        let random = rand::random_range(35..300);
                        local_landmine_list.push_back(random);
                    }
                    landmine_list.lock().expect("Poisoned Mutex").insert(command.channel_id, local_landmine_list);
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

}