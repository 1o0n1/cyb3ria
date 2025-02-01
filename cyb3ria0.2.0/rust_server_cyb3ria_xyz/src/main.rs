mod db;
mod utils;
mod models;
mod handlers;

use warp::Filter;
use dotenv::dotenv;
use log::info;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use handlers::auth::{register_route, login_route};
use handlers::chat::client_connection;

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
        .and(warp::addr::remote())
        .and(warp::query::<std::collections::HashMap<String, String>>()) // Получение параметров из URL
        .map(move |ws: warp::ws::Ws, addr: Option<std::net::SocketAddr>, params: std::collections::HashMap<String, String> | {
            let clients_clone = Arc::clone(&clients_clone);
            let sender_clone = Arc::clone(&sender_clone);
            let peer_addr = addr.expect("Failed to get peer address");
             let username_from_url = params.get("username").map(|s| s.to_string());
            ws.on_upgrade(move |socket| {
                client_connection(socket, clients_clone, sender_clone, peer_addr, username_from_url)
            })
        });

    let register_route = register_route();
    let login_route = login_route();

    let routes = chat_route.or(register_route).or(login_route);
    

    info!("Starting server on 127.0.0.1:8081");
    warp::serve(routes).run(([127, 0, 0, 1], 8081)).await;
}