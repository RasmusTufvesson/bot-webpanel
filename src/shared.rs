use rocket::serde;

#[derive(Debug)]
pub enum Message {
    Send(String),
    ChangeChannel(u64),
    Kill,
    RequestChannels,
    MakeDirectMessageChannel(u64),
    RequestChannelContents,
    RequestMemebers,
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
    pub dms: Vec<DMChannel>,
}

impl ChannelMessage {
    pub fn new(servers: Vec<Server>, dms: Vec<DMChannel>) -> Self {
        Self { servers, dms }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct DiscordMessage {
    content: String,
    author: String,
    attachments: Vec<String>,
}

impl DiscordMessage {
    pub fn new(content: String, author: String, attachments: Vec<String>) -> Self {
        Self { content, author, attachments }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct FullChannel {
    name: String,
    messages: Vec<DiscordMessage>,
    is_dm: bool,
    server: Option<String>
}

impl FullChannel {
    pub fn new(name: String, messages: Vec<DiscordMessage>, is_dm: bool, server: Option<String>) -> Self {
        Self { name, messages, is_dm, server }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct ViewChannelMessage {
    pub channel: Option<FullChannel>,
}

impl ViewChannelMessage {
    pub fn new(channel: Option<FullChannel>) -> Self {
        Self { channel }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Eq, PartialOrd, Ord, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DMChannel {
    pub id: u64,
    pub name: String,
}

impl PartialEq for DMChannel {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl DMChannel {
    pub fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct UsersMessage {
    pub users: Vec<DMChannel>,
}

impl UsersMessage {
    pub fn new(users: Vec<DMChannel>) -> Self {
        Self { users }
    }
}