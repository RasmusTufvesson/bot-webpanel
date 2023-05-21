use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use serenity::async_trait;
use serenity::client::bridge::gateway::ShardManager;
use serenity::model::prelude::{ChannelId, Ready, ChannelType, UserId, Channel};
use serenity::prelude::*;
use rocket::tokio::sync::mpsc::{Receiver, Sender};
use crate::shared::{self, ViewChannelMessage, DiscordMessage, FullChannel};
use fancy_regex::{Regex};

struct Handler {
    loop_running: AtomicBool,
    loop_handler: LoopHandler,
    to_send_recv: Arc<Mutex<Receiver<shared::Message>>>,
    channel_send: Arc<Sender<shared::ChannelMessage>>,
    channel_contents_send: Arc<Sender<shared::ViewChannelMessage>>,
    users_send: Arc<Sender<shared::UsersMessage>>,
    pub dm_channels: Arc<Mutex<Vec<shared::DMChannel>>>,
    link_regex: Regex,
}

#[derive(Clone)]
struct LoopHandler {
    channel_id: Arc<Mutex<Option<ChannelId>>>,
}

struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

#[async_trait]
impl EventHandler for Handler {

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
        
        if !self.loop_running.load(Ordering::Relaxed) {
            let handler = self.loop_handler.clone();
            let channel = Arc::clone(&handler.channel_id);
            let to_send_recv = Arc::clone(&self.to_send_recv);
            let channel_send = Arc::clone(&self.channel_send);
            let channel_contents_send = Arc::clone(&self.channel_contents_send);
            let dm_channels = Arc::clone(&self.dm_channels);
            let users_send = Arc::clone(&self.users_send);
            let link_regex = self.link_regex.clone();
            rocket::tokio::spawn(async move {
                loop {
                    let result = to_send_recv.lock().await.recv().await;
                    match result {
                        Some(message) => {
                            match message {
                                shared::Message::Send(content) => {
                                    if let Some(channel_id) = *channel.lock().await {
                                        send_message(&ctx, channel_id, &content).await;
                                    } else {
                                        eprintln!("No channel specified");
                                    }
                                }
                                shared::Message::ChangeChannel(new_channel) => {
                                    match ChannelId::try_from(new_channel) {
                                        Ok(id) => {
                                            *channel.lock().await = Some(id).into();
                                        }
                                        Err(why) => {
                                            eprintln!("Getting channel failed: {}", why);
                                        }
                                    }
                                }
                                shared::Message::Kill => {
                                    let data = ctx.data.read().await;
                                    let shard_manager = match data.get::<ShardManagerContainer>() {
                                        Some(v) => v,
                                        None => {
                                            panic!("couldnt get shard manager");
                                        },
                                    };
                                    let mut manager = shard_manager.lock().await;
                                    manager.shutdown_all().await;
                                }
                                shared::Message::RequestChannels => {
                                    let mut servers: Vec<shared::Server> = vec![];
                                    if let Ok(guilds) = ready.user.guilds(&ctx.http).await {
                                        for guild in guilds {
                                            if let Ok(guild_channels) = guild.id.channels(&ctx.http).await {
                                                let mut channels: Vec<shared::Channel> = vec![];
                                                for (channel_id, channel) in guild_channels.iter() {
                                                    let channel_type = channel.kind;
                                                    if channel_type == ChannelType::Text || channel_type == ChannelType::Voice {
                                                        channels.push(shared::Channel::new(channel_id.0, channel.name.clone(), channel_type == ChannelType::Voice));
                                                    }
                                                }
                                                servers.push(shared::Server::new(guild.name.clone(), channels))
                                            }
                                        }
                                    }
                                    let channels = shared::ChannelMessage::new(
                                        servers,
                                        dm_channels.lock().await.clone(),
                                    );
                                    match channel_send.send(channels).await {
                                        Ok(_) => {}
                                        Err(why) => {
                                            panic!("couldnt send requested channels: {}", why)
                                        }
                                    }
                                }
                                shared::Message::MakeDirectMessageChannel(user_id) => {
                                    let user_id = UserId::from(user_id);
                                    match user_id.to_user(&ctx.http).await {
                                        Ok(user) => {
                                            match user.create_dm_channel(&ctx.http).await {
                                                Ok(dm) => {
                                                    *channel.lock().await = Some(dm.id);
                                                    let dm_channel = shared::DMChannel::new(user_id.0, user.name);
                                                    if !dm_channels.lock().await.contains(&dm_channel) {
                                                        dm_channels.lock().await.push(dm_channel);
                                                    }
                                                }
                                                Err(why) => {
                                                    eprintln!("Error creating dm: {}", why);
                                                }
                                            }
                                        }
                                        Err(why) => {
                                            eprintln!("Error getting user: {}", why);
                                        }
                                    }
                                }
                                shared::Message::RequestChannelContents => {
                                    if let Some(channel_id) = *channel.lock().await {
                                        match channel_id.messages(&ctx.http, |retriever| retriever.limit(25)).await {
                                            Ok(messages) => {
                                                let mut send_messages: Vec<DiscordMessage> = vec![];
                                                for message in messages {
                                                    let processed_message = link_regex.replace_all(&message.content, r#"<a href="$1">$1</a>"#).to_string();
                                                    let attachments = message.attachments.iter().map(|val| val.url.clone()).collect();
                                                    send_messages.push(DiscordMessage::new(processed_message, message.author.name, attachments));
                                                }
                                                if let Ok(channel) = channel_id.to_channel(&ctx.http).await {
                                                    match channel {
                                                        Channel::Guild(channel) => {
                                                            if let Ok(guild) = channel.guild_id.to_partial_guild(&ctx.http).await {
                                                                let full_channel = FullChannel::new(channel.name, send_messages, false, Some(guild.name));
                                                                match channel_contents_send.send(ViewChannelMessage::new(Some(full_channel))).await {
                                                                    Ok(_) => {}
                                                                    Err(why) => {
                                                                        panic!("couldnt send requested channel messages: {}", why);
                                                                    }
                                                                }
                                                            } else {
                                                                let full_channel = FullChannel::new(channel.name, send_messages, false, None);
                                                                match channel_contents_send.send(ViewChannelMessage::new(Some(full_channel))).await {
                                                                    Ok(_) => {}
                                                                    Err(why) => {
                                                                        panic!("couldnt send requested channel messages: {}", why);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Channel::Private(channel) => {
                                                            let full_channel = FullChannel::new(channel.recipient.name, send_messages, true, None);
                                                            match channel_contents_send.send(ViewChannelMessage::new(Some(full_channel))).await {
                                                                Ok(_) => {}
                                                                Err(why) => {
                                                                    panic!("couldnt send requested channel messages: {}", why);
                                                                }
                                                            }
                                                        }
                                                        _ => {
                                                            match channel_contents_send.send(ViewChannelMessage::new(None)).await {
                                                                Ok(_) => {}
                                                                Err(why) => {
                                                                    panic!("couldnt send requested channel messages: {}", why);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Err(why) => {
                                                eprintln!("Error getting messages: {}", why);
                                                match channel_contents_send.send(ViewChannelMessage::new(None)).await {
                                                    Ok(_) => {}
                                                    Err(why) => {
                                                        panic!("couldnt send requested channel messages: {}", why);
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        eprintln!("No channel specified");
                                        match channel_contents_send.send(ViewChannelMessage::new(None)).await {
                                            Ok(_) => {}
                                            Err(why) => {
                                                panic!("couldnt send requested channel messages: {}", why);
                                            }
                                        }
                                    }
                                }
                                shared::Message::RequestMemebers => {
                                    let mut users: Vec<shared::DMChannel> = vec![];
                                    if let Ok(guilds) = ready.user.guilds(&ctx.http).await {
                                        for guild in guilds {
                                            match guild.id.members(&ctx.http, None, None).await {
                                                Ok(members) => {
                                                    for member in members {
                                                        let user = shared::DMChannel::new(member.user.id.0, member.user.name);
                                                        if !users.contains(&user) && user.id != ready.user.id.0 {
                                                            users.push(user);
                                                        }
                                                    }
                                                }
                                                Err(why) => {
                                                    eprintln!("Couldnt get members of guild: {}", why)
                                                }
                                            }
                                        }
                                    }
                                    let users_message = shared::UsersMessage::new(users);
                                    match users_send.send(users_message).await {
                                        Ok(_) => {}
                                        Err(why) => {
                                            panic!("couldnt send requested members: {}", why)
                                        }
                                    }
                                }
                            }
                        }
                        None => {
                            let data = ctx.data.read().await;
                            let shard_manager = match data.get::<ShardManagerContainer>() {
                                Some(v) => v,
                                None => {
                                    panic!("couldnt get shard manager")
                                },
                            };
                            let mut manager = shard_manager.lock().await;
                            manager.shutdown_all().await;
                        }
                    }
                }
            });

            self.loop_running.swap(true, Ordering::Relaxed);
        }
    }
}

pub async fn main<F>(
    token: String, to_send_recv: Receiver<shared::Message>,
    channel_send: Sender<shared::ChannelMessage>,
    channel_contents_send: Sender<shared::ViewChannelMessage>,
    dm_channels: Vec<shared::DMChannel>,
    save_fn: F,
    users_send: Sender<shared::UsersMessage>,
)
where F: Fn(rocket::tokio::sync::MutexGuard<Vec<shared::DMChannel>>)
{
    let channel_id = Arc::new(Mutex::new(None)); //Arc::new(ChannelId::from(send_channel_id));
    let to_send_recv = Arc::new(Mutex::new(to_send_recv));
    let channel_send = Arc::new(channel_send);
    let channel_contents_send = Arc::new(channel_contents_send);
    let dm_channels: Arc<Mutex<Vec<shared::DMChannel>>> = Arc::new(Mutex::new(dm_channels));
    let users_send = Arc::new(users_send);
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let link_regex = Regex::new(r"(?<![^ \n])(https?://.*?)(?![^ \n])").unwrap();
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            loop_running: AtomicBool::new(false),
            loop_handler: LoopHandler { channel_id: channel_id },
            to_send_recv,
            channel_send,
            channel_contents_send,
            dm_channels: Arc::clone(&dm_channels),
            users_send,
            link_regex,
        })
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
    }

    if let Err(why) = client.start().await {
        eprintln!("Client error: {:?}", why);
    }
    let guard: rocket::tokio::sync::MutexGuard<Vec<shared::DMChannel>> = dm_channels.lock().await;
    save_fn(guard);
}

async fn send_message(ctx: &Context, channel_id: ChannelId, message: &str) {
    if let Err(why) = channel_id.say(&ctx.http, message).await {
        eprintln!("Error sending message: {:?}", why);
    }
}