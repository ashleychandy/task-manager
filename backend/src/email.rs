use std::time::Duration;

use axum::{extract::State, Json};
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{error::AppError, AppState, Config};

const SUBJECT: &str = "Your verification code";

/// The most recent OTP, kept in memory for development only.
#[derive(Clone, Serialize)]
pub struct DevOtp {
    pub recipient: String,
    pub login_challenge_id: Uuid,
    pub code: String,
}

async fn deliver(
    config: &Config,
    recipient: &str,
    code: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (Some(host), Some(username), Some(password), Some(from)) = (
        &config.smtp_host,
        &config.smtp_username,
        &config.smtp_password,
        &config.smtp_from_email,
    ) else {
        return Err("SMTP is not configured".into());
    };

    let message = Message::builder()
        .from(from.parse::<Mailbox>()?)
        .to(recipient.parse::<Mailbox>()?)
        .subject(SUBJECT)
        .body(format!(
            "Your verification code is {code}. It expires in 5 minutes."
        ))?;

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
        .port(config.smtp_port)
        .credentials(Credentials::new(username.clone(), password.clone()))
        .timeout(Some(Duration::from_secs(10)))
        .build();

    mailer.send(message).await?;
    Ok(())
}

/// Send the OTP and record the outcome. A failed send is logged, not returned,
/// so the login flow still works locally without live SMTP.
pub async fn send_otp(
    state: &AppState,
    recipient: &str,
    login_challenge_id: Uuid,
    code: &str,
) -> Result<(), AppError> {
    let status = match deliver(&state.config, recipient, code).await {
        Ok(()) => "sent",
        Err(e) => {
            eprintln!("failed to send OTP email to {recipient}: {e}");
            "failed"
        }
    };

    sqlx::query("INSERT INTO email_logs (recipient, subject, delivery_status) VALUES ($1, $2, $3)")
        .bind(recipient)
        .bind(SUBJECT)
        .bind(status)
        .execute(&state.pool)
        .await?;

    if state.config.app_env == "development" {
        *state.dev_otp.lock().unwrap() = Some(DevOtp {
            recipient: recipient.to_string(),
            login_challenge_id,
            code: code.to_string(),
        });
    }
    Ok(())
}

/// GET /dev/email-logs/latest: development only.
pub async fn latest_dev_email(State(state): State<AppState>) -> Result<Json<DevOtp>, AppError> {
    if state.config.app_env != "development" {
        return Err(AppError::NotFound("Not found".to_string()));
    }
    let latest = state.dev_otp.lock().unwrap().clone();
    latest
        .map(Json)
        .ok_or_else(|| AppError::NotFound("No email sent yet".to_string()))
}
