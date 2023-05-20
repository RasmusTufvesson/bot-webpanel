use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use serenity::async_trait;
use serenity::client::bridge::gateway::ShardManager;
use serenity::model::prelude::{ChannelId, Ready, ChannelType, UserId, Channel};
use serenity::prelude::*;
use rocket::tokio::sync::mpsc::{Receiver, Sender};
use crate::shared::{self, ViewChannelMessage, DiscordMessage, FullChannel};

struct Handler {
    loop_running: AtomicBool,
    loop_handler: LoopHandler,
    to_send_recv: Arc<Mutex<Receiver<shared::Message>>>,
    channel_send: Arc<Sender<shared::ChannelMessage>>,
    channel_contents_send: Arc<Sender<shared::ViewChannelMessage>>,
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
                                    panic!("Bot shutdown")
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
                                                    send_messages.push(DiscordMessage::new(message.content, message.author.name))
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
                            panic!("channel closed");
                        }
                    }
                }
            });

            self.loop_running.swap(true, Ordering::Relaxed);
        }
    }
}

pub async fn main(token: String, to_send_recv: Receiver<shared::Message>, channel_send: Sender<shared::ChannelMessage>, channel_contents_send: Sender<shared::ViewChannelMessage>) {
    let channel_id = Arc::new(Mutex::new(None)); //Arc::new(ChannelId::from(send_channel_id));
    let to_send_recv = Arc::new(Mutex::new(to_send_recv));
    let channel_send = Arc::new(channel_send);
    let channel_contents_send = Arc::new(channel_contents_send);
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            loop_running: AtomicBool::new(false),
            loop_handler: LoopHandler { channel_id: channel_id },
            to_send_recv,
            channel_send,
            channel_contents_send,
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
}

async fn send_message(ctx: &Context, channel_id: ChannelId, message: &str) {
    if let Err(why) = channel_id.say(&ctx.http, message).await {
        eprintln!("Error sending message: {:?}", why);
    }
}