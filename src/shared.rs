use rocket::serde;


#[derive(Debug)]
pub enum Message {
    Send(String),
    ChangeChannel(u64),
    Kill,
    RequestChannels,
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