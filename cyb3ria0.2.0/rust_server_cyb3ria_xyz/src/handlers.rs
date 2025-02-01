use warp::ws::{WebSocket, Message};
use futures_util::stream::{StreamExt};
use futures_util::SinkExt;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::sync::Mutex as TokioMutex;
use log::{info, error, debug};
use serde::{Deserialize, Serialize};
use crate::db::{
    save_message_to_db, send_message_history, save_user_to_db, find_user_by_username,
    save_device_to_db, save_session_to_db, find_device_by_ip_mac
};
use crate::utils::generate_client_id;
use warp::{Filter, Rejection, http::StatusCode};
use crate::models::{User, Device, Session};
use bcrypt::{hash, DEFAULT_COST, verify};
use uuid::Uuid;
use std::net::SocketAddr;
use chrono::{Utc, Duration};
use tokio::time::{Duration as TokioDuration, interval};

type Clients = Arc<Mutex<std::collections::HashMap<String, usize>>>;
type Sender = Arc<Mutex<broadcast::Sender<String>>>;

#[derive(Deserialize, Serialize, Debug)]
struct ClientMessage {
    username: String,
    message: String,
    ip: String,
    mac: String
}

#[derive(Deserialize, Serialize, Debug)]
struct RegistrationData {
    username: String,
    password: String,
    repeat_password: String,
    invitation_code: String,
    ip_address: String,
    mac_address: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
struct RegistrationResponse {
    message: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct LoginData {
    username: String,
    password: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct LoginResponse {
    message: String,
    username: String,
}

pub async fn client_connection(ws: WebSocket, clients: Clients, sender: Sender, peer_addr: SocketAddr, username_from_url: Option<String>) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let client_ws_sender = Arc::new(TokioMutex::new(client_ws_sender));
    let username = username_from_url.unwrap_or_else(|| peer_addr.ip().to_string());

    let client_id = {
        let mut clients = clients.lock().unwrap();
        let client_id = generate_client_id();
        clients.insert(client_id.clone(), 0);
        client_id
    };

    info!("New client connected with ID: {}, username: {}", client_id, username);

    if let Err(e) = send_message_history(client_ws_sender.clone()).await {
        error!("Failed to send message history: {}", e);
    }
    
    let mut rx = sender.lock().unwrap().subscribe();
    let username_clone = username.clone();
    let clients_clone = Arc::clone(&clients);
    let client_id_clone = client_id.clone();

    let ping_interval = TokioDuration::from_secs(30);
    let mut ping_timer = interval(ping_interval);

    // Клонируем Arc для использования в асинхронной задаче
    let client_ws_sender_task = Arc::clone(&client_ws_sender);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = ping_timer.tick() => {
                    if let Err(e) = client_ws_sender_task.lock().await.send(Message::ping(vec![])).await {
                        error!("Failed to send ping message: {}", e);
                        let mut clients = clients_clone.lock().unwrap();
                        clients.remove(&client_id_clone);
                        info!("Client disconnected with ID: {}, username: {}", client_id_clone, username_clone);
                        break;
                    }
                }
                Ok(message) = rx.recv() => {
                    debug!("Broadcasting message: {}", message);
                    if let Err(e) = client_ws_sender_task.lock().await.send(Message::text(message)).await {
                        error!("Failed to send message: {}", e);
                        let mut clients = clients_clone.lock().unwrap();
                        clients.remove(&client_id_clone);
                        info!("Client disconnected with ID: {}, username: {}", client_id_clone, username_clone);
                        break;
                    }
                }
            }
        }
    });
    
    while let Some(result) = client_ws_rcv.next().await {
        let msg = if let Ok(msg) = result {
            if msg.is_text() {
                let msg_str = msg.to_str().unwrap().to_owned();
                debug!("Received raw message: {}", msg_str);
                let client_message: ClientMessage = match serde_json::from_str(&msg_str) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Failed to deserialize message: {}", e);
                        continue;
                    }
                };
                debug!("Received message from client {}: {}", client_message.username, client_message.message);
                let user = match find_user_by_username(&client_message.username).await {
                    Ok(user) => user,
                    Err(e) => {
                        error!("Failed to find user: {}", e);
                        continue;
                    }
                };

                if let Err(e) = save_message_to_db(&client_message.message, user.user_uuid).await {
                    error!("Failed to save message to database: {}", e);
                }

                let formatted_message = format!("{}: {}", client_message.username, client_message.message);
                formatted_message
            } else if msg.is_close() {
                info!("Client disconnected with ID: {}, username: {}", client_id, username);
                let mut clients = clients.lock().unwrap();
                clients.remove(&client_id);
                break;
            } else {
                continue;
            }
        } else {
            break;
        };

        if let Err(e) = sender.lock().unwrap().send(msg) {
            error!("Failed to send message to broadcast: {}", e);
        }
    }

    if let Err(e) = client_ws_sender.lock().await.close().await {
        error!("Failed to close client connection: {}", e);
    }

    let mut clients = clients.lock().unwrap();
    clients.remove(&client_id);
    info!("Client disconnected with ID: {}, username: {}", client_id, username);
}

pub async fn register_handler(registration: RegistrationData, _peer_addr: SocketAddr) -> Result<impl warp::Reply, Rejection> {
    debug!("Received registration request: {:?}", registration);

    if registration.username.is_empty() || registration.password.is_empty() || registration.repeat_password.is_empty() || registration.invitation_code.is_empty() {
        error!("All fields must be filled.");
        let response = RegistrationResponse { message: "All fields must be filled.".to_string() };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::BAD_REQUEST,
        ));
    }

    if registration.password != registration.repeat_password {
        error!("Passwords do not match.");
        let response = RegistrationResponse { message: "Passwords do not match.".to_string() };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::BAD_REQUEST,
        ));
    }

    let password_hash = match hash(registration.password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            error!("Failed to hash password: {}", e);
            let response = RegistrationResponse { message: "Failed to hash password.".to_string() };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };

    let user_uuid = Uuid::new_v4();

    let user = User {
        username: registration.username,
        password_hash: password_hash,
        invitation_code: registration.invitation_code,
        user_uuid,
    };

    match save_user_to_db(user).await {
        Ok(_) => {
            info!("User registered successfully.");

            let device = Device {
                device_id: Uuid::new_v4(),
                user_uuid,
                ip_address: registration.ip_address,
            };

            if let Err(e) = save_device_to_db(device).await {
                error!("Failed to save device to database: {}", e);
            }

            let response = RegistrationResponse { message: "User registered successfully".to_string() };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        },
        Err(e) => {
            error!("Failed to save user to database: {}", e);
            let response = RegistrationResponse { message: "Failed to save user to database.".to_string() };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub fn register_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("register"))
        .and(warp::body::json())
        .and(warp::addr::remote())
        .and_then(|registration: RegistrationData, addr: Option<SocketAddr>| async move {
            let peer_addr = addr.expect("Failed to get peer address");
            register_handler(registration, peer_addr).await
        })
}

pub async fn login_handler(login: LoginData, peer_addr: SocketAddr) -> Result<impl warp::Reply, Rejection> {
    debug!("Received login request: {:?}", login);

    if login.username.is_empty() || login.password.is_empty() {
        error!("All fields must be filled.");
        let response = LoginResponse { message: "All fields must be filled.".to_string(), username: "".to_string() };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::BAD_REQUEST,
        ));
    }

    let user = match find_user_by_username(&login.username).await {
        Ok(user) => user,
        Err(e) => {
            error!("Failed to find user: {}", e);
            let response = LoginResponse { message: "Failed to find user.".to_string(), username: "".to_string() };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::UNAUTHORIZED,
            ));
        }
    };

    if !verify(&login.password, &user.password_hash).unwrap_or(false) {
        error!("Invalid password.");
        let response = LoginResponse { message: "Invalid password.".to_string(), username: "".to_string() };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::UNAUTHORIZED,
        ));
    }

    let device = match find_device_by_ip_mac(&peer_addr.ip().to_string(), None).await {
        Ok(Some(device)) => device,
        Ok(None) => {
            let device = Device {
                device_id: Uuid::new_v4(),
                user_uuid: user.user_uuid,
                ip_address: peer_addr.ip().to_string(),
            };
            if let Err(e) = save_device_to_db(device.clone()).await {
                error!("Failed to save device to database: {}", e);
            }
            device
        },
        Err(e) => {
            error!("Failed to find device: {}", e);
            let response = LoginResponse { message: "Failed to find device.".to_string(), username: "".to_string() };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ));
        }
    };
    
    let session = Session {
        session_id: Uuid::new_v4(),
        user_uuid: user.user_uuid,
        device_id: device.device_id,
        expires_at: Some(Utc::now() + Duration::hours(1)),
    };

    if let Err(e) = save_session_to_db(session).await {
        error!("Failed to save session to database: {}", e);
    }

    info!("User logged in successfully: {}", login.username);
    let response = LoginResponse { message: "User logged in successfully.".to_string(), username: login.username.to_string() };
    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}

pub fn login_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("login"))
        .and(warp::body::json())
        .and(warp::addr::remote())
        .and_then(|login: LoginData, addr: Option<SocketAddr>| async move {
            let peer_addr = addr.expect("Failed to get peer address");
            login_handler(login, peer_addr).await
        })
}