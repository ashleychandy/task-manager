use backend::{router, AppState, Config};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let config = Config::from_env().unwrap_or_else(|e| {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    });

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to the database");

    let addr = format!("127.0.0.1:{}", config.app_port);
    let app = router(AppState::new(pool, config));

    let listener = TcpListener::bind(&addr)
        .await
        .expect("failed to bind the listen address");
    println!("listening on http://{addr}");
    axum::serve(listener, app).await.expect("server error");
}
