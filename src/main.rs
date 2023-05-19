use std::{fs, sync::Arc};

use rocket::{
    tokio,
    serde, futures::lock::Mutex,
};

mod server;
mod bot;
mod shared;

#[derive(serde::Deserialize)]
#[serde(crate = "rocket::serde")]
struct Config {
    token: String,
    invite_link: String,
    bot_name: String,
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let config = fs::read_to_string("config.toml")
        .expect("Should have been able to read the file");
    let config: Config = toml::from_str(&config).unwrap();

    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let (tx_channels, rx_channels) = tokio::sync::mpsc::channel(100);
    let rx_channels_arc = Arc::new(Mutex::new(rx_channels));
    let (tx_channel, rx_channel) = tokio::sync::mpsc::channel(100);
    let rx_channel_arc = Arc::new(Mutex::new(rx_channel));

    let server = tokio::spawn(server::main(tx, rx_channels_arc, rx_channel_arc, config.bot_name, config.invite_link));
    bot::main(config.token, rx, tx_channels, tx_channel).await;
    server.await.unwrap()?;

    Ok(())
}
