use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub invitation_code: String,
    pub user_uuid: Uuid,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Device {
    pub device_id: Uuid,
    pub user_uuid: Uuid,
    pub ip_address: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Session {
    pub session_id: Uuid,
    pub user_uuid: Uuid,
    pub device_id: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
}