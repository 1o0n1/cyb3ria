use warp::{Filter, Rejection, http::StatusCode};
use crate::models::{User, Device, Session};
use bcrypt::{hash, DEFAULT_COST, verify};
use uuid::Uuid;
use std::net::SocketAddr;
use chrono::{Utc, Duration};
use validator::{Validate, ValidationErrors, ValidationError};
use log::{info, error, debug};
use serde::{Deserialize, Serialize};
use crate::db::{save_user_to_db, find_user_by_username, save_device_to_db, save_session_to_db, find_device_by_ip_mac};
use crate::utils::generate_csrf_token;
use std::borrow::Cow;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RegistrationData {
    pub username: String,
    pub password: String,
    pub repeat_password: String,
    pub invitation_code: String,
    pub ip_address: String,
    pub mac_address: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RegistrationResponse {
    pub message: String,
    pub csrf_token: String, // Добавляем CSRF токен
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LoginData {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LoginResponse {
    pub message: String,
    pub username: String,
    pub csrf_token: String, // Добавляем CSRF токен
}

impl Validate for RegistrationData {
    fn validate(&self) -> Result<(), ValidationErrors> {
       let mut errors = ValidationErrors::new();

        if self.username.len() < 3 || self.username.len() > 16 {
            let mut error = ValidationError::new("length");
            error.message = Some("Username must be between 3 and 16 characters".to_string().into());
            errors.add("username", error);
        }
        if self.password.len() < 6 || self.password.len() > 16 {
             let mut error = ValidationError::new("length");
            error.message = Some("Password must be between 6 and 16 characters".to_string().into());
            errors.add("password", error);
        }
        if self.repeat_password.len() < 6 || self.repeat_password.len() > 16 {
            let mut error = ValidationError::new("length");
            error.message = Some("Repeat password must be between 6 and 16 characters".to_string().into());
             errors.add("repeat_password", error);
        }
         if self.invitation_code.len() < 3 || self.invitation_code.len() > 16 {
            let mut error = ValidationError::new("length");
            error.message = Some("Invitation code must be between 3 and 16 characters".to_string().into());
            errors.add("invitation_code", error);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Validate for LoginData {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();

         if self.username.len() < 3 || self.username.len() > 16 {
            let mut error = ValidationError::new("length");
            error.message = Some("Username must be between 3 and 16 characters".to_string().into());
            errors.add("username", error);
        }
        if self.password.len() < 6 || self.password.len() > 16 {
             let mut error = ValidationError::new("length");
            error.message = Some("Password must be between 6 and 16 characters".to_string().into());
            errors.add("password", error);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn map_validation_errors(errors: ValidationErrors) -> String {
    let mut result = String::new();
    for (_, field_errors) in errors.field_errors() {
        for error in field_errors {
           result.push_str(&format!("{} ", error.message.as_ref().unwrap_or(&Cow::from("Invalid value")).to_string()));
        }
    }
    result.trim().to_string()
}

async fn validate_csrf_token(header: Option<String>) -> Result<(), CsrfError> {
     if let Some(header_token) = header {
            if !header_token.is_empty(){
                 return Ok(());
            }
     }
     Err(CsrfError::InvalidToken)
 }

#[derive(Debug)]
enum CsrfError {
    InvalidToken,
}

impl warp::reject::Reject for CsrfError {}

pub async fn register_handler(registration: RegistrationData,  header: Option<String> , _peer_addr: SocketAddr) -> Result<impl warp::Reply, Rejection> {
    debug!("Received registration request: {:?}", registration);

       let result = validate_csrf_token(header).await;
          if let Err(e) = result {
            error!("CSRF validation error: {:?}", e);
                return  Err(warp::reject::custom(e));
        }
    // Валидация данных
   if let Err(errors) = registration.validate() {
         error!("Validation errors: {:?}", errors);
         let error_message = map_validation_errors(errors);
         let response = RegistrationResponse { message: error_message, csrf_token: "".to_string() };
         return Ok(warp::reply::with_status(
             warp::reply::json(&response),
             StatusCode::BAD_REQUEST,
        ));
     }
    

    if registration.password != registration.repeat_password {
        error!("Passwords do not match.");
        let response = RegistrationResponse { message: "Passwords do not match.".to_string(), csrf_token: "".to_string() };
        return Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::BAD_REQUEST,
        ));
    }

    let password_hash = match hash(registration.password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            error!("Failed to hash password: {}", e);
            let response = RegistrationResponse { message: "Failed to hash password.".to_string(), csrf_token: "".to_string() };
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

            let csrf_token = generate_csrf_token();

            let response = RegistrationResponse { message: "User registered successfully".to_string(), csrf_token };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::OK,
            ))
        },
        Err(e) => {
            error!("Failed to save user to database: {}", e);
            let response = RegistrationResponse { message: "Failed to save user to database.".to_string(), csrf_token: "".to_string() };
            Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub async fn login_handler(login: LoginData,  header: Option<String>,  peer_addr: SocketAddr) -> Result<impl warp::Reply, Rejection> {
    debug!("Received login request: {:?}", login);
        let result = validate_csrf_token(header).await;
          if let Err(e) = result {
            error!("CSRF validation error: {:?}", e);
                return  Err(warp::reject::custom(e));
        }
    // Валидация данных
   if let Err(errors) = login.validate() {
        error!("Validation errors: {:?}", errors);
        let error_message = map_validation_errors(errors);
        let response = LoginResponse { message: error_message , username: "".to_string(), csrf_token: "".to_string()};
         return Ok(warp::reply::with_status(
            warp::reply::json(&response),
           StatusCode::BAD_REQUEST,
        ));
    }

    let user = match find_user_by_username(&login.username).await {
        Ok(user) => user,
        Err(e) => {
            error!("Failed to find user: {}", e);
            let response = LoginResponse { message: "Failed to find user.".to_string(), username: "".to_string(), csrf_token: "".to_string() };
            return Ok(warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::UNAUTHORIZED,
            ));
        }
    };

    if !verify(&login.password, &user.password_hash).unwrap_or(false) {
        error!("Invalid password.");
        let response = LoginResponse { message: "Invalid password.".to_string(), username: "".to_string(), csrf_token: "".to_string() };
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
            let response = LoginResponse { message: "Failed to find device.".to_string(), username: "".to_string() , csrf_token: "".to_string() };
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
    let csrf_token = generate_csrf_token();

    info!("User logged in successfully: {}", login.username);
    let response = LoginResponse { message: "User logged in successfully.".to_string(), username: login.username.to_string(), csrf_token };
    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}


pub fn register_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("register"))
         .and(warp::header::optional::<String>("X-CSRF-Token"))
        .and(warp::body::json())
        .and(warp::addr::remote())
        .and_then(|header: Option<String>, registration: RegistrationData, addr: Option<SocketAddr>| async move {
            let peer_addr = addr.expect("Failed to get peer address");
            register_handler(registration,  header, peer_addr).await
        })
}


pub fn login_route() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api")
        .and(warp::path("login"))
         .and(warp::header::optional::<String>("X-CSRF-Token"))
        .and(warp::body::json())
        .and(warp::addr::remote())
        .and_then(|header: Option<String>, login: LoginData, addr: Option<SocketAddr>| async move {
            let peer_addr = addr.expect("Failed to get peer address");
            login_handler(login, header, peer_addr).await
        })
}