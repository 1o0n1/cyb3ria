use warp::ws::WebSocket;
use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::sync::Mutex as TokioMutex;
use log::{info, error, debug};
use serde::{Deserialize, Serialize};
use crate::db::{save_message_to_db, send_message_history, save_user_to_db};
use crate::utils::generate_client_id;
use warp::{Filter, Rejection, http::StatusCode};
use crate::models::User;
use bcrypt::{hash, DEFAULT_COST};

type Clients = Arc<Mutex<std::collections::HashMap<String, usize>>>;
type Sender = Arc<Mutex<broadcast::Sender<String>>>;

#[derive(Deserialize, Serialize)]
struct ClientMessage {
   client_id: String,
   message: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct RegistrationData {
   username: String,
   password: String,
   repeat_password: String,
   invitation_code: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct RegistrationResponse {
   message: String,
}

pub async fn client_connection(ws: WebSocket, clients: Clients, sender: Sender) {
   let (client_ws_sender, mut client_ws_rcv) = ws.split();
   let client_ws_sender = Arc::new(TokioMutex::new(client_ws_sender));
   let mut rx = sender.lock().unwrap().subscribe();

   let client_id = {
       let mut clients = clients.lock().unwrap();
       let client_id = generate_client_id();
       clients.insert(client_id.clone(), 0);
       client_id
   };

   info!("New client connected with ID: {}", client_id);

   // Загрузка истории сообщений из базы данных и отправка новому клиенту
   if let Err(e) = send_message_history(client_ws_sender.clone()).await {
       error!("Failed to send message history: {}", e);
   }

   // Отправка сообщений всем клиентам
   let client_ws_sender_clone = Arc::clone(&client_ws_sender);
   let clients_clone = Arc::clone(&clients);
   let client_id_clone = client_id.clone();
   tokio::spawn(async move {
       while let Ok(message) = rx.recv().await {
           debug!("Broadcasting message: {}", message);
           if let Err(e) = client_ws_sender_clone.lock().await.send(warp::ws::Message::text(message)).await {
               error!("Failed to send message: {}", e);
               // Удаление клиента из списка при ошибке отправки
               let mut clients = clients_clone.lock().unwrap();
               clients.remove(&client_id_clone);
               info!("Client disconnected with ID: {}", client_id_clone);
               break;
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
               debug!("Received message from client {}: {}", client_message.client_id, client_message.message);

               // Запись сообщения в базу данных
               if let Err(e) = save_message_to_db(&client_message.message).await {
                   error!("Failed to save message to database: {}", e);
               }

               client_message.message
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
   info!("Client disconnected with ID: {}", client_id);
}

pub async fn register_handler(registration: RegistrationData) -> Result<impl warp::Reply, Rejection> {
   debug!("Received registration request: {:?}", registration);

   // Валидация данных
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

   // Хеширование пароля
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

   // Создание модели пользователя
    let user = User {
        username: registration.username,
        password_hash: password_hash,
        invitation_code: registration.invitation_code,
    };

   // Сохранение в базу данных
   match save_user_to_db(user).await {
         Ok(_) => {
             info!("User registered successfully.");
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
      .and_then(register_handler)
}