use std::{fs, sync::Arc};
use rocket::{
    tokio,
    serde, futures::lock::{Mutex},
};
use ciborium;

mod server;
mod bot;
mod shared;

#[derive(serde::Deserialize)]
#[serde(crate = "rocket::serde")]
struct Config {
    token: String,
    invite_link: String,
    bot_name: String,
    save_path: String,
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let config = fs::read_to_string("config.toml")
        .expect("Should have been able to read the file");
    let config: Config = toml::from_str(&config).unwrap();
    let data = load(&config.save_path);

    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let (tx_channels, rx_channels) = tokio::sync::mpsc::channel(100);
    let rx_channels_arc = Arc::new(Mutex::new(rx_channels));
    let (tx_channel, rx_channel) = tokio::sync::mpsc::channel(100);
    let rx_channel_arc = Arc::new(Mutex::new(rx_channel));
    let (tx_users, rx_users) = tokio::sync::mpsc::channel(100);
    let rx_users_arc = Arc::new(Mutex::new(rx_users));

    let server = tokio::spawn(server::main(tx, rx_channels_arc, rx_channel_arc, rx_users_arc, config.bot_name, config.invite_link));
    bot::main(config.token, rx, tx_channels, tx_channel, data, |channels| save(channels, &config.save_path), tx_users).await;
    server.await.unwrap()?;

    Ok(())
}

fn save(channels: rocket::tokio::sync::MutexGuard<Vec<shared::DMChannel>>, path: &str) {
    let file = fs::File::create(path).unwrap();
    ciborium::into_writer(&(*channels), file).unwrap();
}

fn load(path: &str) -> Vec<shared::DMChannel> {
    match std::path::Path::new(path).exists() {
        true => {
            ciborium::from_reader(fs::File::open(path).unwrap()).unwrap()
        }
        false => {
            vec![]
        }
    }
}