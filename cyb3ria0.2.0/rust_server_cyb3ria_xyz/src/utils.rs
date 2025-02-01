use uuid::Uuid;

pub fn generate_client_id() -> String {
    Uuid::new_v4().to_string()
}
