use rocket::serde;

#[derive(Debug)]
pub enum Message {
    Send(String),
    ChangeChannel(u64),
    Kill,
    RequestChannels,
    MakeDirectMessageChannel(u64),
    RequestChannelContents,
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Channel {
    id: u64,
    name: String,
    is_voice: bool,
}

impl Channel {
    pub fn new(id: u64, name: String, is_voice: bool) -> Self {
        Self { id, name, is_voice }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Server {
    name: String,
    channels: Vec<Channel>,
}

impl Server {
    pub fn new(name: String, channels: Vec<Channel>) -> Self {
        Self { name, channels }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct ChannelMessage {
    pub servers: Vec<Server>,
}

impl ChannelMessage {
    pub fn new(servers: Vec<Server>) -> Self {
        Self { servers }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct DiscordMessage {
    content: String,
    author: String,
}

impl DiscordMessage {
    pub fn new(content: String, author: String) -> Self {
        Self { content, author }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct FullChannel {
    name: String,
    messages: Vec<DiscordMessage>,
    is_dm: bool,
}

impl FullChannel {
    pub fn new(name: String, messages: Vec<DiscordMessage>, is_dm: bool) -> Self {
        Self { name, messages, is_dm }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct ViewChannelMessage {
    pub channel: Option<FullChannel>
}

impl ViewChannelMessage {
    pub fn new(channel: Option<FullChannel>) -> Self {
        Self { channel }
    }
}