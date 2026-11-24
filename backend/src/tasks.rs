use std::{collections::HashSet, sync::Arc};

use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::{AuthUser, UserInfo},
    error::AppError,
    AppState,
};

const STATUSES: [&str; 3] = ["todo", "in_progress", "done"];
const PRIORITIES: [&str; 3] = ["low", "medium", "high"];

#[derive(Serialize, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub created_by_id: Uuid,
    pub assigned_to_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateTask {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: String,
    pub priority: String,
}

#[derive(Deserialize)]
pub struct AssignRequest {
    pub task_ids: Vec<Uuid>,
    pub assignee_email: String,
}

// The cached view-my-tasks payload. The `cache` field is added when responding.
#[derive(Serialize, FromRow)]
pub struct MyTask {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub assigned_to: String,
}

#[derive(Serialize)]
pub struct Summary {
    pub total_assigned_tasks: usize,
}

#[derive(Serialize)]
pub struct MyTasks {
    pub user: UserInfo,
    pub tasks: Vec<MyTask>,
    pub summary: Summary,
}

/// POST /tasks (admin)
pub async fn create_task(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreateTask>,
) -> Result<(StatusCode, Json<Task>), AppError> {
    user.require_admin()?;

    let title = req.title.trim();
    if title.is_empty() {
        return Err(AppError::BadRequest("Title must not be empty".to_string()));
    }
    if !STATUSES.contains(&req.status.as_str()) {
        return Err(AppError::BadRequest(
            "Status must be one of: todo, in_progress, done".to_string(),
        ));
    }
    if !PRIORITIES.contains(&req.priority.as_str()) {
        return Err(AppError::BadRequest(
            "Priority must be one of: low, medium, high".to_string(),
        ));
    }

    let task = sqlx::query_as::<_, Task>(
        "INSERT INTO tasks (title, description, status, priority, created_by_id) \
         VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(title)
    .bind(&req.description)
    .bind(&req.status)
    .bind(&req.priority)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(task)))
}

/// GET /tasks (admin)
pub async fn list_tasks(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, AppError> {
    user.require_admin()?;

    let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks ORDER BY created_at, id")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(json!({ "tasks": tasks })))
}

/// POST /tasks/assign (admin)
pub async fn assign_tasks(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<AssignRequest>,
) -> Result<Json<Value>, AppError> {
    user.require_admin()?;

    let mut task_ids = req.task_ids;
    if task_ids.is_empty() {
        return Err(AppError::BadRequest("task_ids must not be empty".to_string()));
    }
    task_ids.sort();
    task_ids.dedup();

    // Any early return below drops `tx`, which rolls back.
    let mut tx = state.pool.begin().await?;

    let assignee_id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE email = $1")
        .bind(&req.assignee_email)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("Assignee not found".to_string()))?;

    // Lock the tasks and read their previous assignees; this also detects unknown ids.
    let existing = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
        "SELECT id, assigned_to_id FROM tasks WHERE id = ANY($1) FOR UPDATE",
    )
    .bind(&task_ids)
    .fetch_all(&mut *tx)
    .await?;
    if existing.len() != task_ids.len() {
        return Err(AppError::NotFound("One or more tasks not found".to_string()));
    }

    sqlx::query("UPDATE tasks SET assigned_to_id = $1, updated_at = now() WHERE id = ANY($2)")
        .bind(assignee_id)
        .bind(&task_ids)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // Invalidate after commit: the new assignee and everyone who lost a task.
    let mut affected: HashSet<Uuid> = existing.iter().filter_map(|(_, prev)| *prev).collect();
    affected.insert(assignee_id);
    for id in affected {
        state.cache.invalidate(&id).await;
    }

    Ok(Json(json!({
        "assigned": task_ids.len(),
        "assignee_email": req.assignee_email
    })))
}

fn respond(payload: &MyTasks, hit: bool) -> Json<Value> {
    Json(json!({
        "user": payload.user,
        "tasks": payload.tasks,
        "summary": payload.summary,
        "cache": { "hit": hit }
    }))
}

/// GET /tasks/view-my-tasks (any logged-in user)
pub async fn view_my_tasks(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Value>, AppError> {
    if let Some(payload) = state.cache.get(&user.id).await {
        return Ok(respond(&payload, true));
    }

    let tasks = sqlx::query_as::<_, MyTask>(
        "SELECT t.id, t.title, t.status, t.priority, u.email AS assigned_to \
         FROM tasks t JOIN users u ON u.id = t.assigned_to_id \
         WHERE t.assigned_to_id = $1 \
         ORDER BY t.created_at, t.id",
    )
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;

    let payload = Arc::new(MyTasks {
        user: UserInfo {
            email: user.email,
            role: user.role,
        },
        summary: Summary {
            total_assigned_tasks: tasks.len(),
        },
        tasks,
    });
    // Known limitation: a miss that races with an assign can re-cache stale data.
    // Acceptable for single-instance local development.
    state.cache.insert(user.id, payload.clone()).await;
    Ok(respond(&payload, false))
}
