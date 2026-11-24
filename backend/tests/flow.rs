use std::collections::HashSet;

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use backend::{router, AppState, Config};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

/// Send one request through the router in-process and return (status, JSON body).
async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(json) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };

    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// Log in and read the code from the dev endpoint. Returns (login_challenge_id, code).
async fn start_login(app: &Router, email: &str, password: &str) -> (String, String) {
    let (status, body) = call(
        app,
        "POST",
        "/auth/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.get("access_token").is_none(), "login must not return a JWT");
    let challenge = body["login_challenge_id"].as_str().unwrap().to_string();

    let (status, dev) = call(app, "GET", "/dev/email-logs/latest", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dev["login_challenge_id"].as_str().unwrap(), challenge);
    (challenge, dev["code"].as_str().unwrap().to_string())
}

async fn verify(app: &Router, challenge: &str, code: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        "/auth/verify-2fa",
        None,
        Some(json!({ "login_challenge_id": challenge, "code": code })),
    )
    .await
}

/// Full login: password, then code, then token.
async fn login_token(app: &Router, email: &str, password: &str) -> String {
    let (challenge, code) = start_login(app, email, password).await;
    let (status, body) = verify(app, &challenge, &code).await;
    assert_eq!(status, StatusCode::OK);
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn full_flow() {
    let mut config = Config::from_env().expect("config (is backend/.env filled in?)");
    config.app_env = "development".to_string();
    let admin_email = config.admin_email.clone();
    let admin_password = config.admin_password.clone();
    let bond_email = config.james_bond_email.clone();
    let bond_password = config.james_bond_password.clone();

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("database connection");
    sqlx::query("TRUNCATE users, tasks, login_challenges, email_logs CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    let app = router(AppState::new(pool.clone(), config));

    // 1. Seed users.
    let (status, _) = call(&app, "POST", "/seed/users", None, None).await;
    assert_eq!(status, StatusCode::OK);

    // 2. Admin login returns a challenge and no token (checked inside start_login).
    let (challenge, code) = start_login(&app, &admin_email, &admin_password).await;

    // 3. Wrong code is rejected without consuming the challenge; the right code then works.
    let wrong = if code == "000000" { "000001" } else { "000000" };
    let (status, _) = verify(&app, &challenge, wrong).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = verify(&app, &challenge, &code).await;
    assert_eq!(status, StatusCode::OK);
    let admin_token = body["access_token"].as_str().unwrap().to_string();
    let admin = admin_token.as_str();

    // 4. A used code is rejected, and so is an expired challenge.
    let (status, body) = verify(&app, &challenge, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].as_str().unwrap().contains("used"));

    let (challenge, code) = start_login(&app, &admin_email, &admin_password).await;
    sqlx::query("UPDATE login_challenges SET expires_at = now() - interval '1 minute' WHERE id = $1")
        .bind(Uuid::parse_str(&challenge).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = verify(&app, &challenge, &code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].as_str().unwrap().contains("expired"));

    // 5. Admin creates 5 tasks and lists them.
    let mut ids = Vec::new();
    for i in 1..=5 {
        let (status, task) = call(
            &app,
            "POST",
            "/tasks",
            Some(admin),
            Some(json!({
                "title": format!("Task {i}"),
                "description": "",
                "status": "todo",
                "priority": "medium"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        ids.push(task["id"].as_str().unwrap().to_string());
    }
    let (status, body) = call(&app, "GET", "/tasks", Some(admin), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tasks"].as_array().unwrap().len(), 5);

    // 6. Admin assigns 3 tasks to James Bond.
    let (status, body) = call(
        &app,
        "POST",
        "/tasks/assign",
        Some(admin),
        Some(json!({ "task_ids": &ids[..3], "assignee_email": bond_email })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["assigned"], 3);

    // 7. James Bond logs in; staff cannot create or assign; no token is 401.
    let bond_token = login_token(&app, &bond_email, &bond_password).await;
    let bond = bond_token.as_str();
    let new_task = json!({ "title": "Nope", "description": "", "status": "todo", "priority": "low" });
    let (status, _) = call(&app, "POST", "/tasks", Some(bond), Some(new_task.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "POST",
        "/tasks/assign",
        Some(bond),
        Some(json!({ "task_ids": &ids[3..4], "assignee_email": bond_email })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(&app, "POST", "/tasks", None, Some(new_task)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 8. view-my-tasks returns exactly the 3 assigned tasks: a miss first, then a hit.
    let (status, body) = call(&app, "GET", "/tasks/view-my-tasks", Some(bond), None).await;
    assert_eq!(status, StatusCode::OK);
    let got: HashSet<String> = body["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect();
    let want: HashSet<String> = ids[..3].iter().cloned().collect();
    assert_eq!(got, want);
    assert_eq!(body["summary"]["total_assigned_tasks"], 3);
    assert_eq!(body["cache"]["hit"], false);

    let (_, body) = call(&app, "GET", "/tasks/view-my-tasks", Some(bond), None).await;
    assert_eq!(body["cache"]["hit"], true);

    // 9. Reassigning the 3 tasks invalidates the cache.
    let (status, _) = call(
        &app,
        "POST",
        "/tasks/assign",
        Some(admin),
        Some(json!({ "task_ids": &ids[..3], "assignee_email": bond_email })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = call(&app, "GET", "/tasks/view-my-tasks", Some(bond), None).await;
    assert_eq!(body["cache"]["hit"], false);
    assert_eq!(body["summary"]["total_assigned_tasks"], 3);
}
