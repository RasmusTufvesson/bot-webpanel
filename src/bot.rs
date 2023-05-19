use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::sync::Arc;
use serenity::async_trait;
use serenity::client::bridge::gateway::ShardManager;
use serenity::model::prelude::{ChannelId, Ready, ChannelType};
use serenity::prelude::*;
use rocket::tokio::sync::mpsc::{Receiver, error::TryRecvError, Sender};
use crate::shared;

struct Handler {
    loop_running: AtomicBool,
    loop_handler: LoopHandler,
    to_send_recv: Arc<Mutex<Receiver<shared::Message>>>,
    channel_send: Arc<Sender<Vec<shared::Server>>>,
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
            rocket::tokio::spawn(async move {
                loop {
                    let result = to_send_recv.lock().await.try_recv();
                    match result {
                        Ok(message) => {
                            match message {
                                shared::Message::Send(content) => {
                                    if let Some(channel_id) = *channel.lock().await {
                                        send_message(&ctx, channel_id, &content).await;
                                    } else {
                                        eprintln!("No channel specified")
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
                                            panic!("couldnt get shard manager")
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
                                    match channel_send.send(servers).await {
                                        Ok(_) => {}
                                        Err(why) => {
                                            panic!("couldnt send requested channels: {}", why)
                                        }
                                    }
                                }
                            }
                        },
                        Err(TryRecvError::Disconnected) => {
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
                        },
                        _ => {}
                    }
                    rocket::tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });

            self.loop_running.swap(true, Ordering::Relaxed);
        }
    }
}

pub async fn main(token: String, to_send_recv: Receiver<shared::Message>, channel_send: Sender<Vec<shared::Server>>) {
    let channel_id = Arc::new(Mutex::new(None)); //Arc::new(ChannelId::from(send_channel_id));
    let to_send_recv = Arc::new(Mutex::new(to_send_recv));
    let channel_send = Arc::new(channel_send);
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            loop_running: AtomicBool::new(false),
            loop_handler: LoopHandler { channel_id: channel_id },
            to_send_recv,
            channel_send,
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
    println!("is correct: {}", ChannelId::from(1025817646433833023) == channel_id);
    println!("correct: {}", ChannelId::from(1025817646433833023));
    println!("unknown: {}", channel_id);
    if let Err(why) = channel_id.say(&ctx.http, message).await {
        eprintln!("Error sending message: {:?}", why);
    }
}