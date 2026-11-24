pub mod auth;
pub mod email;
pub mod error;
pub mod tasks;

use std::{
    env,
    str::FromStr,
    sync::{Arc, Mutex},
};

use axum::{
    http::{header, HeaderValue, Method},
    routing::{get, post},
    Router,
};
use moka::future::Cache;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::{email::DevOtp, tasks::MyTasks};

pub struct Config {
    pub database_url: String,
    pub app_env: String,
    pub app_port: u16,
    pub jwt_secret: String,
    pub jwt_expires_in_seconds: u64,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub smtp_from_email: Option<String>,
    pub admin_email: String,
    pub admin_password: String,
    pub james_bond_email: String,
    pub james_bond_password: String,
}

/// Reads a variable, treating an empty value (as in `.env.example`) as unset.
fn var(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.is_empty())
}

fn required(key: &str) -> Result<String, String> {
    var(key).ok_or_else(|| format!("{key} must be set"))
}

fn parsed<T: FromStr>(key: &str, default: T) -> Result<T, String> {
    match var(key) {
        Some(v) => v
            .parse::<T>()
            .map_err(|_| format!("{key} has an invalid value")),
        None => Ok(default),
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            app_env: var("APP_ENV").unwrap_or_else(|| "development".to_string()),
            app_port: parsed("APP_PORT", 8080)?,
            jwt_secret: required("JWT_SECRET")?,
            jwt_expires_in_seconds: parsed("JWT_EXPIRES_IN_SECONDS", 3600)?,
            smtp_host: var("SMTP_HOST"),
            smtp_port: parsed("SMTP_PORT", 587)?,
            smtp_username: var("SMTP_USERNAME"),
            smtp_password: var("SMTP_PASSWORD"),
            smtp_from_email: var("SMTP_FROM_EMAIL"),
            admin_email: var("ADMIN_EMAIL").unwrap_or_else(|| "admin@example.com".to_string()),
            admin_password: required("ADMIN_PASSWORD")?,
            james_bond_email: var("JAMES_BOND_EMAIL")
                .unwrap_or_else(|| "jamesbond@example.com".to_string()),
            james_bond_password: required("JAMES_BOND_PASSWORD")?,
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    /// view-my-tasks payloads keyed by user id. No TTL: entries are invalidated on assign.
    pub cache: Cache<Uuid, Arc<MyTasks>>,
    /// Latest OTP, development only. The plaintext code never touches the database.
    pub dev_otp: Arc<Mutex<Option<DevOtp>>>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Self {
        Self {
            pool,
            config: Arc::new(config),
            cache: Cache::new(10_000),
            dev_otp: Arc::new(Mutex::new(None)),
        }
    }
}

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("http://localhost:5173"))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    Router::new()
        .route("/seed/users", post(auth::seed_users))
        .route("/auth/login", post(auth::login))
        .route("/auth/verify-2fa", post(auth::verify_2fa))
        .route("/dev/email-logs/latest", get(email::latest_dev_email))
        .route("/tasks", post(tasks::create_task).get(tasks::list_tasks))
        .route("/tasks/assign", post(tasks::assign_tasks))
        .route("/tasks/view-my-tasks", get(tasks::view_my_tasks))
        .layer(cors)
        .with_state(state)
}
