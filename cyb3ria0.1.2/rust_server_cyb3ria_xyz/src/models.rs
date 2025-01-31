use serde::{Serialize, Deserialize};

#[derive(Deserialize, Serialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub invitation_code: String,
}
