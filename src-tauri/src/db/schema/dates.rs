use chrono::NaiveDate;
use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

pub async fn init_table(conn: &mut PoolConnection<Sqlite>) -> Result<(), sqlx::Error> {
    let _ = sqlx::query(
        "CREATE TABLE dates (
            date_str TEXT
        );",
    )
    .execute(&mut **conn)
    .await?;

    Ok(())
}

pub async fn insert(conn: &mut PoolConnection<Sqlite>, date: NaiveDate) -> Result<(), sqlx::Error> {
    let _ = sqlx::query("INSERT INTO dates (date_str) values (?1);")
        .bind(date)
        .execute(&mut **conn)
        .await?;

    Ok(())
}
