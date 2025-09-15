use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use color_eyre::eyre::{self, Report, WrapErr};
use console::style;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::db::{DbConnection, Transaction, TransactionsByCategory};

async fn get_transactions(
    mut conn: DbConnection,
) -> Result<Json<Vec<Transaction>>, (StatusCode, String)> {
    conn.get_transactions()
        .await
        .map(Json)
        .map_err(internal_eyre)
}

async fn get_expenses_over_time(
    mut conn: DbConnection,
) -> Result<Json<TransactionsByCategory>, (StatusCode, String)> {
    conn.get_expenses_over_time()
        .await
        .map(Json)
        .map_err(internal_eyre)
}

async fn get_income_over_time(
    mut conn: DbConnection,
) -> Result<Json<TransactionsByCategory>, (StatusCode, String)> {
    conn.get_income_over_time()
        .await
        .map(Json)
        .map_err(internal_eyre)
}

pub fn internal_eyre(err: Report) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

pub fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

pub async fn run(db_pool: SqlitePool) -> eyre::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:3030").await.unwrap();

    let api = Router::new()
        .route("/transactions", get(get_transactions))
        .route("/expenses", get(get_expenses_over_time))
        .route("/income", get(get_income_over_time))
        .with_state(db_pool);

    let app = Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new("assets"));

    println!(
        "Starting server at {}",
        style("http://127.0.0.1:3030").bold().bright().blue()
    );

    axum::serve(listener, app)
        .await
        .wrap_err("Failed to start server")
}
