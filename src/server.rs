use std::sync::Arc;

use rocket::{
    get,
    routes,
    post,
    serde::{Deserialize, json::Json},
    tokio::sync::mpsc,
    State,
    Shutdown,
    fs::{FileServer, relative}, futures::lock::Mutex,
};
use rocket_dyn_templates::{Template, context};
use crate::shared;

struct BotInfo {
    bot_name: String,
    invite_link: String,
}

#[get("/")]
fn homepage(bot_info: &State<BotInfo>) -> Template {
    Template::render("homepage", context! {
        name: &bot_info.inner().bot_name,
        link: &bot_info.inner().invite_link,
    })
}

#[get("/change")]
async fn change(sender: &State<mpsc::Sender<shared::Message>>, receiver: &State<Arc<Mutex<mpsc::Receiver<Vec<shared::Server>>>>>, bot_info: &State<BotInfo>) -> Template {
    sender.send(
        shared::Message::RequestChannels
    ).await.unwrap();
    let mut receiver = receiver.lock().await;
    let servers = receiver.recv().await.unwrap();
    Template::render("change_channel", context! {
        servers: servers,
        name: &bot_info.inner().bot_name,
    })
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct Message<'r> {
    content: &'r str,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct ChangeChannel<'r> {
    channel: &'r str, // parse to u64
}

#[post("/send", data = "<message>", format = "application/json")]
async fn send(sender: &State<mpsc::Sender<shared::Message>>, message: Json<Message<'_>>) {
    sender.send(
        shared::Message::Send(message.content.to_owned())
    ).await.unwrap();
}

#[post("/change", data = "<new_channel>", format = "application/json")]
async fn change_channel(sender: &State<mpsc::Sender<shared::Message>>, new_channel: Json<ChangeChannel<'_>>) {
    sender.send(
        shared::Message::ChangeChannel(match new_channel.channel.parse::<u64>() {
            Ok(val) => {val}
            Err(why) => {
                eprintln!("Failed to parse channel id: {}", why);
                return;
            }
        })
    ).await.unwrap();
}

#[post("/kill")]
async fn kill(sender: &State<mpsc::Sender<shared::Message>>, shutdown: Shutdown) {
    println!("killing");
    sender.send(
        shared::Message::Kill
    ).await.unwrap();
    shutdown.notify();
}

pub async fn main(sender: mpsc::Sender<shared::Message>, receiver: Arc<Mutex<mpsc::Receiver<Vec<shared::Server>>>>, bot_name: String, invite_link: String) -> Result<(), rocket::Error> {
    let _rocket = rocket::build()
        .manage(sender)
        .manage(receiver)
        .manage(BotInfo {bot_name, invite_link})
        .mount("/", routes![homepage, send, change_channel, kill, change])
        .mount("/static", FileServer::from(relative!("static")))
        .attach(Template::fairing())
        .launch()
        .await?;

    Ok(())
}