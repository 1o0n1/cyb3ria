mod handlers;
mod db;
mod utils;
mod models;

use warp::Filter;
use dotenv::dotenv;
use log::info;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use handlers::client_connection;

type Clients = Arc<Mutex<std::collections::HashMap<String, usize>>>;
type Sender = Arc<Mutex<broadcast::Sender<String>>>;

#[tokio::main]
async fn main() {
    // Загрузка переменных окружения из .env файла
    dotenv().ok();

    // Инициализация логирования
    env_logger::init();

    // Логирование начала работы сервера
    info!("Initializing server...");

    let clients: Clients = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let sender: Sender = Arc::new(Mutex::new(broadcast::channel(100).0));
    let clients_clone = Arc::clone(&clients);
    let sender_clone = Arc::clone(&sender);

    let chat_route = warp::path("api")
        .and(warp::path("ws"))
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let clients_clone = Arc::clone(&clients_clone);
            let sender_clone = Arc::clone(&sender_clone);
            ws.on_upgrade(move |socket| {
                client_connection(socket, clients_clone, sender_clone)
            })
        });

    info!("Starting server on 127.0.0.1:8081");
    warp::serve(chat_route).run(([127, 0, 0, 1], 8081)).await;
}
