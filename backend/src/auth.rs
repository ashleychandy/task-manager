use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use axum::{
    extract::{FromRequestParts, State},
    http::{header::AUTHORIZATION, request::Parts},
    Json,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{email::send_otp, error::AppError, AppState, Config};

// ---------- Argon2 helpers (used for passwords and OTPs) ----------

pub fn hash_secret(secret: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(e.to_string()))?
        .to_string();
    Ok(hash)
}

pub fn verify_secret(secret: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(secret.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// ---------- JWT ----------

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    email: String,
    role: String,
    exp: u64,
}

fn issue_token(user: &User, config: &Config) -> Result<String, AppError> {
    let claims = Claims {
        sub: user.id,
        email: user.email.clone(),
        role: user.role.clone(),
        exp: Utc::now().timestamp() as u64 + config.jwt_expires_in_seconds,
    };
    encode(
        &Header::default(), // HS256
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}

// ---------- Extractor ----------

pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub role: String,
}

impl AuthUser {
    pub fn require_admin(&self) -> Result<(), AppError> {
        if self.role == "admin" {
            Ok(())
        } else {
            Err(AppError::Forbidden("Admin access required".to_string()))
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let unauthorized = || AppError::Unauthorized("Missing or invalid token".to_string());

        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(unauthorized)?;

        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &Validation::default(), // HS256, validates exp
        )
        .map_err(|_| unauthorized())?;

        Ok(AuthUser {
            id: data.claims.sub,
            email: data.claims.email,
            role: data.claims.role,
        })
    }
}

// ---------- Handlers ----------

#[derive(Serialize)]
pub struct UserInfo {
    pub email: String,
    pub role: String,
}

#[derive(FromRow)]
struct User {
    id: Uuid,
    email: String,
    hashed_password: String,
    role: String,
}

#[derive(FromRow)]
struct Challenge {
    user_id: Uuid,
    otp_hash: String,
    expired: bool,
    consumed: bool,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub login_challenge_id: Uuid,
    pub code: String,
}

fn invalid_credentials() -> AppError {
    AppError::Unauthorized("Invalid email or password".to_string())
}

/// POST /seed/users: idempotently create the Admin and James Bond users.
pub async fn seed_users(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let c = &state.config;
    let seeds = [
        ("Admin", &c.admin_email, &c.admin_password, "admin"),
        ("James Bond", &c.james_bond_email, &c.james_bond_password, "staff"),
    ];

    let mut users = Vec::new();
    for (name, address, password, role) in seeds {
        sqlx::query(
            "INSERT INTO users (full_name, email, hashed_password, role) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (email) DO UPDATE SET \
                 full_name = EXCLUDED.full_name, \
                 hashed_password = EXCLUDED.hashed_password, \
                 role = EXCLUDED.role",
        )
        .bind(name)
        .bind(address)
        .bind(hash_secret(password)?)
        .bind(role)
        .execute(&state.pool)
        .await?;
        users.push(json!({ "email": address, "role": role }));
    }
    Ok(Json(json!({ "users": users })))
}

/// POST /auth/login: check the password, then email a one-time code. No JWT yet.
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<Value>, AppError> {
    let user = sqlx::query_as::<_, User>(
        "SELECT id, email, hashed_password, role FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(invalid_credentials)?;

    if !verify_secret(&req.password, &user.hashed_password) {
        return Err(invalid_credentials());
    }

    let code = format!("{:06}", rand::random_range(0..1_000_000u32));

    let challenge_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO login_challenges (user_id, otp_hash, expires_at) \
         VALUES ($1, $2, now() + interval '5 minutes') RETURNING id",
    )
    .bind(user.id)
    .bind(hash_secret(&code)?)
    .fetch_one(&state.pool)
    .await?;

    send_otp(&state, &user.email, challenge_id, &code).await?;

    Ok(Json(json!({
        "message": "OTP sent successfully",
        "login_challenge_id": challenge_id,
        "expires_in": 300
    })))
}

/// POST /auth/verify-2fa: exchange a valid code for a JWT.
pub async fn verify_2fa(
    State(state): State<AppState>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<Value>, AppError> {
    let mut tx = state.pool.begin().await?;

    // Lock the challenge row so a code can only be redeemed once, even under concurrency.
    let challenge = sqlx::query_as::<_, Challenge>(
        "SELECT user_id, otp_hash, expires_at <= now() AS expired, \
                consumed_at IS NOT NULL AS consumed \
         FROM login_challenges WHERE id = $1 FOR UPDATE",
    )
    .bind(req.login_challenge_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Unknown login challenge".to_string()))?;

    // Any early return below drops `tx`, which rolls back. A wrong code consumes nothing.
    if challenge.consumed {
        return Err(AppError::Unauthorized("Login code already used".to_string()));
    }
    if challenge.expired {
        return Err(AppError::Unauthorized("Login code expired".to_string()));
    }
    if !verify_secret(&req.code, &challenge.otp_hash) {
        return Err(AppError::Unauthorized("Invalid login code".to_string()));
    }

    sqlx::query("UPDATE login_challenges SET consumed_at = now() WHERE id = $1")
        .bind(req.login_challenge_id)
        .execute(&mut *tx)
        .await?;

    let user = sqlx::query_as::<_, User>(
        "SELECT id, email, hashed_password, role FROM users WHERE id = $1",
    )
    .bind(challenge.user_id)
    .fetch_one(&mut *tx)
    .await?;

    // Sign before committing so a signing failure doesn't burn the code.
    let token = issue_token(&user, &state.config)?;
    tx.commit().await?;

    Ok(Json(json!({
        "access_token": token,
        "user": UserInfo { email: user.email, role: user.role }
    })))
}
