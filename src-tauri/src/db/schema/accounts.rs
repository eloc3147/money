use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

pub async fn init_table(conn: &mut PoolConnection<Sqlite>) -> Result<(), sqlx::Error> {
    let _ = sqlx::query(
        "CREATE TABLE accounts (
            name TEXT NOT NULL,
            UNIQUE(name)
        );",
    )
    .execute(&mut **conn)
    .await?;

    Ok(())
}

pub async fn insert(conn: &mut PoolConnection<Sqlite>, name: &str) -> Result<(), sqlx::Error> {
    let _ = sqlx::query("INSERT INTO accounts (name) values (?1) ON CONFLICT(name) DO NOTHING;")
        .bind(name)
        .execute(&mut **conn)
        .await?;

    Ok(())
}
