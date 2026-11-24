# Task Management

A full-stack task management app with Rust/Axum, PostgreSQL/SQLx, Argon2 password hashing,
JWT authentication, email-based 2FA, role-based access, and an in-memory per-user cache.

## Prerequisites

- Rust stable
- Node.js and npm
- PostgreSQL

## Backend setup

```bash
cd backend
cp .env.example .env
```

Fill in `DATABASE_URL`, `JWT_SECRET`, `ADMIN_PASSWORD`, and `JAMES_BOND_PASSWORD`.
Apply the SQL migration using your preferred PostgreSQL migration workflow, for example:

```bash
psql "$DATABASE_URL" -f migrations/0001_init.sql
```

Start the API:

```bash
cargo run
```

The API listens on `http://localhost:8080` by default.

## Frontend setup

```bash
cd frontend
cp .env.example .env
npm install
npm run dev
```

Open `http://localhost:5173`.

## Seed users

```bash
curl -X POST http://localhost:8080/seed/users
```

Development credentials:

- Admin: `admin@example.com` / `admin@123`
- James Bond: `jamesbond@example.com` / `james@123`

## Development OTP

The login endpoint validates credentials and creates a challenge but does not return a JWT:

```bash
curl -i -X POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@example.com","password":"admin@123"}'
```

Retrieve the latest development code:

```bash
curl http://localhost:8080/dev/email-logs/latest
```

Verify the challenge:

```bash
curl -i -X POST http://localhost:8080/auth/verify-2fa \
  -H 'Content-Type: application/json' \
  -d '{"login_challenge_id":"CHALLENGE_ID","code":"123456"}'
```

The response returns `access_token`. Add it as `Authorization: Bearer TOKEN` for
protected endpoints.

## Validation flow

1. Seed users.
2. Log in as Admin and verify the 2FA code.
3. Create five tasks:

```bash
curl -X POST http://localhost:8080/tasks \
  -H 'Authorization: Bearer ADMIN_TOKEN' \
  -H 'Content-Type: application/json' \
  -d '{"title":"Task 1","description":"","status":"todo","priority":"high"}'
```

Repeat for Task 2 through Task 5.

4. Assign exactly three task IDs:

```bash
curl -X POST http://localhost:8080/tasks/assign \
  -H 'Authorization: Bearer ADMIN_TOKEN' \
  -H 'Content-Type: application/json' \
  -d '{"task_ids":["TASK_ID_1","TASK_ID_2","TASK_ID_3"],"assignee_email":"jamesbond@example.com"}'
```

5. Log in as James Bond and verify his 2FA code.
6. Verify staff creation is forbidden:

```bash
curl -i -X POST http://localhost:8080/tasks \
  -H 'Authorization: Bearer JAMES_BOND_TOKEN' \
  -H 'Content-Type: application/json' \
  -d '{"title":"Blocked","description":"","status":"todo","priority":"low"}'
```

Expected status: `403 Forbidden`.

7. Call the assigned-task endpoint twice:

```bash
curl -H 'Authorization: Bearer JAMES_BOND_TOKEN' \
  http://localhost:8080/tasks/view-my-tasks
```

The first response should contain exactly three tasks and `"hit": false`. The
second identical request should contain `"hit": true`.

Example final response:

```json
{
  "user": {
    "email": "jamesbond@example.com",
    "role": "staff"
  },
  "tasks": [
    {
      "id": "46b28480-fcc9-4af2-9b29-e747217791c9",
      "title": "Task 1",
      "status": "todo",
      "priority": "medium",
      "assigned_to": "jamesbond@example.com"
    },
    {
      "id": "a2d0c432-c4c8-44da-9811-a6042e20bfe9",
      "title": "Task 2",
      "status": "todo",
      "priority": "medium",
      "assigned_to": "jamesbond@example.com"
    },
    {
      "id": "8002a351-4a8c-4632-9630-5aef0ec5338d",
      "title": "Task 3",
      "status": "todo",
      "priority": "medium",
      "assigned_to": "jamesbond@example.com"
    }
  ],
  "summary": {
    "total_assigned_tasks": 3
  },
  "cache": {
    "hit": false
  }
}
```

The same endpoint called again returns `cache.hit: true`.

## Cache behavior

The cache is per logged-in user and held in memory via Moka. It is not shared across
multiple backend processes or persisted across restarts. Assignment invalidates cache
entries for both the previous assignee and the new assignee.

## Tests

```bash
cd backend
cargo test
```

The integration test covers seeding, password login, 2FA challenge verification,
incorrect and expired/reused codes, five tasks, three assignments, staff 403,
assigned-task filtering, and cache hits.

Frontend:

```bash
cd frontend
npm run build
npm run lint
```