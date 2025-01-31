use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::env;
use chrono::NaiveDateTime;
use log::{info, error, debug};
use bcrypt::{hash, verify, DEFAULT_COST};

#[derive(Queryable, Serialize, Debug)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
}

#[derive(Deserialize, Debug)]
pub struct InputUser {
    pub username: String,
    pub password: String,
}

#[derive(Queryable, Serialize, Debug)]
pub struct ChatMessage {
    pub id: i32,
    pub user_id: i32,
    pub message: String,
    pub timestamp: NaiveDateTime,
}

#[derive(Insertable, Deserialize, Debug)]
#[diesel(table_name = chat_messages)]
pub struct NewChatMessage {
    pub user_id: i32,
    pub message: String,
}

#[derive(Deserialize, Debug)]
pub struct InputMessage {
    pub user_id: i32,
    pub message: String,
}

fn establish_connection_pool() -> Pool<ConnectionManager<PgConnection>> {
    dotenv::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder().build(manager).expect("Failed to create pool.")
}

fn get_connection(pool: &Pool<ConnectionManager<PgConnection>>) -> Result<PooledConnection<ConnectionManager<PgConnection>>, actix_web::Error> {
    pool.get().map_err(actix_web::error::ErrorInternalServerError)
}

diesel::table! {
    chat_messages (id) {
        id -> Int4,
        user_id -> Int4,
        message -> Text,
        timestamp -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        username -> Varchar,
        password_hash -> Varchar,
    }
}

fn insert_user(
    conn: &mut PgConnection,
    new_user: NewUser,
) -> Result<(), diesel::result::Error> {
    use crate::users::dsl::*;
    debug!("Inserting user: {:?}", new_user);
    diesel::insert_into(users)
        .values(&new_user)
        .execute(conn)?;

    Ok(())
}

fn insert_message(
    conn: &mut PgConnection,
    new_message: NewChatMessage,
) -> Result<(), diesel::result::Error> {
    use crate::chat_messages::dsl::*;
    debug!("Inserting message: {:?}", new_message);
    diesel::insert_into(chat_messages)
        .values(&new_message)
        .execute(conn)?;

    Ok(())
}

async fn post_user(
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    form: web::Json<InputUser>,
) -> actix_web::Result<impl Responder> {
    info!("Received data: {:?}", form);
    let mut conn = get_connection(&pool)?;
    let hashed_password = hash(form.password.clone(), DEFAULT_COST).expect("Failed to hash password");
    let new_user = NewUser {
        username: form.username.clone(),
        password_hash: hashed_password,
    };
    match insert_user(&mut conn, new_user) {
        Ok(_) => {
            info!("User insert was successful!");
            Ok(HttpResponse::Ok().json("User registered successfully"))
        }
        Err(e) => {
            error!("Error inserting user: {:?}", e);
            Ok(HttpResponse::InternalServerError().body(format!("Error inserting user: {:?}", e)))
        }
    }
}

async fn post_login(
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    form: web::Json<InputUser>,
) -> actix_web::Result<impl Responder> {
    info!("Received data: {:?}", form);
    let mut conn = get_connection(&pool)?;
    use crate::users::dsl::*;
    let user: Result<User, diesel::result::Error> = users
        .filter(username.eq(&form.username))
        .first(&mut conn);

    match user {
        Ok(user) => {
            if verify(&form.password, &user.password_hash).expect("Failed to verify password") {
                info!("Login successful!");
                Ok(HttpResponse::Ok().json("You have successfully logged in"))
            } else {
                info!("Login failed: Incorrect password");
                Ok(HttpResponse::Unauthorized().body("Incorrect password"))
            }
        }
        Err(_) => {
            info!("Login failed: User not found");
            Ok(HttpResponse::Unauthorized().body("User not found"))
        }
    }
}

async fn post_chat_message(
    pool: web::Data<Pool<ConnectionManager<PgConnection>>>,
    form: web::Json<InputMessage>,
) -> actix_web::Result<impl Responder> {
    info!("Received data: {:?}", form);
    let mut conn = get_connection(&pool)?;
    let new_message = NewChatMessage {
        user_id: form.user_id,
        message: form.message.clone(),
    };
    match insert_message(&mut conn, new_message) {
        Ok(_) => {
            info!("Data insert was successful!");
            Ok(HttpResponse::Ok().json("Data inserted successfully"))
        }
        Err(e) => {
            error!("Error inserting data: {:?}", e);
            Ok(HttpResponse::InternalServerError().body(format!("Error inserting data: {:?}", e)))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));
    dotenv::dotenv().ok();
    let pool = establish_connection_pool();
    let port = env::var("PORT").unwrap_or("8081".to_string()).parse::<u16>().expect("Failed to parse port");
    info!("Starting server on port: {}", port);
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .route("/api/register", web::post().to(post_user))
            .route("/api/login", web::post().to(post_login))
            .route("/api/chat", web::post().to(post_chat_message))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
