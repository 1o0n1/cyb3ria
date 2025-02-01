use uuid::Uuid;
use rand::Rng;
use rand::distributions::Alphanumeric;

pub fn generate_client_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn generate_csrf_token() -> String {
 let mut rng = rand::thread_rng();
 (0..32)
      .map(|_| rng.sample(Alphanumeric) as char)
      .collect()
}