use tokio_postgres::{NoTls, types::Type};
use std::error::Error as StdError;
use tokio::sync::Mutex as TokioMutex;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use warp::ws::WebSocket;
use std::sync::Arc;
use log::{error, debug};
use crate::models::{User, Device, Session};
use uuid::Uuid;
use std::net::IpAddr;
use chrono::{DateTime, Utc};
use tokio_postgres::types::{ToSql, accepts};
use std::result::Result;
use bytes::BytesMut;

// Обертка для DateTime<Utc>, чтобы обойти "orphan rule"
#[derive(Debug)]
struct Timestamp(DateTime<Utc>);

impl ToSql for Timestamp {
    accepts!(TIMESTAMP, TIMESTAMPTZ);
    tokio_postgres::types::to_sql_checked!();

    fn to_sql(&self, _type: &Type, out: &mut BytesMut) -> Result<tokio_postgres::types::IsNull, Box<dyn StdError + Sync + Send>> {
        let timestamp = self.0.timestamp();
        (timestamp).to_sql(_type, out).map_err(|e| e.into())
    }
}

/// Сохраняет сообщение в базу данных
pub async fn save_message_to_db(message: &str, user_uuid: Uuid) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Saving message to database: {}, from user: {}", message, user_uuid);

    client.execute(
        "INSERT INTO messages (message, user_uuid) VALUES ($1, $2)",
        &[&message, &user_uuid],
    )
    .await?;

    Ok(())
}

/// Отправляет историю сообщений клиенту
pub async fn send_message_history(client_ws_sender: Arc<TokioMutex<SplitSink<WebSocket, warp::ws::Message>>>) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Fetching message history from database");

    let rows = client.query("SELECT message, user_uuid FROM messages ORDER BY timestamp ASC", &[]).await?;

    for row in rows {
        let message: String = row.get(0);
         // Получаем user_uuid, обрабатывая возможность NULL значения
        let user_uuid: Option<Uuid> = row.get(1);
        
         let formatted_message = match user_uuid {
            Some(uuid) => {
                // Получаем имя пользователя по user_uuid
                let username_result = client.query_one("SELECT username FROM users WHERE user_uuid = $1", &[&uuid]).await;
                 match username_result {
                    Ok(row) => {
                         let username: String = row.get(0);
                         format!("{}: {}", username, message)
                     },
                    Err(e) => {
                       error!("Failed to get username from database: {}", e);
                         format!("Unknown User: {}", message)
                    }
                }
            }
             None => format!("Unknown User: {}", message), // Обработка NULL значения
        };
        
        if let Err(e) = client_ws_sender.lock().await.send(warp::ws::Message::text(formatted_message)).await {
            error!("Failed to send message history: {}", e);
            return Err(Box::new(e));
        }
    }

    Ok(())
}

/// Сохраняет пользователя в базу данных
pub async fn save_user_to_db(user: User) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Saving user to database: {}", user.username);

    client.execute(
        "INSERT INTO users (username, password_hash, invitation_code, user_uuid) VALUES ($1, $2, $3, $4)",
        &[&user.username, &user.password_hash, &user.invitation_code, &user.user_uuid],
    )
    .await?;

    Ok(())
}

/// Ищет пользователя по имени
pub async fn find_user_by_username(username: &str) -> Result<User, Box<dyn StdError + Send + Sync>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Finding user in database by username: {}", username);

    let row = client
        .query_one("SELECT username, password_hash, invitation_code, user_uuid FROM users WHERE username = $1", &[&username])
        .await?;

    let user = User {
        username: row.get(0),
        password_hash: row.get(1),
        invitation_code: row.get(2),
        user_uuid: row.get(3),
    };

    Ok(user)
}

/// Сохраняет устройство в базу данных
pub async fn save_device_to_db(device: Device) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Saving device to database: {:?}", device);

    // Преобразуем IP-адрес в тип IpAddr
    let ip_address: IpAddr = device.ip_address.parse().map_err(|e| {
        error!("Failed to parse IP address: {}", e);
        e
    })?;

    client.execute(
        "INSERT INTO devices (device_id, user_uuid, ip_address) VALUES ($1, $2, $3)",
        &[&device.device_id, &device.user_uuid, &ip_address],
    )
    .await?;

    Ok(())
}

/// Ищет устройство по IP-адресу
pub async fn find_device_by_ip_mac(ip_address: &str, _mac_address: Option<&str>) -> Result<Option<Device>, Box<dyn StdError + Send + Sync>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Finding device by IP: {}", ip_address);

    // Преобразуем IP-адрес в тип IpAddr
    let ip_addr: IpAddr = ip_address.parse().map_err(|e| {
        error!("Failed to parse IP address: {}", e);
        e
    })?;


    let row = client.query_opt(
            "SELECT device_id, user_uuid, ip_address FROM devices WHERE ip_address = $1",
            &[&ip_addr],
        )
        .await?;
    

    if let Some(row) = row {
        let device = Device {
            device_id: row.get(0),
            user_uuid: row.get(1),
            ip_address: row.get::<_, IpAddr>(2).to_string(),  // Преобразуем обратно в строку
           
        };
        Ok(Some(device))
    } else {
        Ok(None)
    }
}

/// Сохраняет сессию в базу данных
pub async fn save_session_to_db(session: Session) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Saving session to database: {:?}", session);

    let expires_at = session.expires_at.map(Timestamp);

    client.execute(
        "INSERT INTO sessions (session_id, user_uuid, device_id, expires_at) VALUES ($1, $2, $3, $4)",
        &[&session.session_id, &session.user_uuid, &session.device_id, &expires_at],
    )
    .await?;

    Ok(())
}