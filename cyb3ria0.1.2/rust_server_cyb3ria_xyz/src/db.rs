use tokio_postgres::{NoTls};
use std::error::Error as StdError;
use tokio::sync::Mutex as TokioMutex;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use warp::ws::WebSocket;
use std::sync::Arc;
use log::{error, debug};
use crate::models::User;

pub async fn save_message_to_db(message: &str) -> Result<(), Box<dyn StdError>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on a background task.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Saving message to database: {}", message);

    client.execute(
        "INSERT INTO messages (message) VALUES ($1)",
        &[&message],
    )
    .await?;

    Ok(())
}

pub async fn send_message_history(client_ws_sender: Arc<TokioMutex<SplitSink<WebSocket, warp::ws::Message>>>) -> Result<(), Box<dyn StdError>> {
    let (client, connection) =
        tokio_postgres::connect("host=localhost user=cyb3ria password=!Abs123 dbname=cyb3ria_db", NoTls)
            .await
            .expect("Failed to connect to database");

    // The connection object performs the actual communication with the database,
    // so spawn it off to run on a background task.
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    debug!("Fetching message history from database");

    let rows = client.query("SELECT message FROM messages ORDER BY timestamp ASC", &[]).await?;

    for row in rows {
        let message: String = row.get(0);
        if let Err(e) = client_ws_sender.lock().await.send(warp::ws::Message::text(message)).await {
            error!("Failed to send message history: {}", e);
            return Err(Box::new(e));
        }
    }

    Ok(())
}

pub async fn save_user_to_db(user: User) -> Result<(), Box<dyn StdError>> {
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
        "INSERT INTO users (username, password_hash, invitation_code) VALUES ($1, $2, $3)",
        &[&user.username, &user.password_hash, &user.invitation_code],
    )
    .await?;

    Ok(())
}