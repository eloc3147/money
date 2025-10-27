use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

#[derive(Debug)]
pub struct Category {
    pub base_category: String,
    pub category: String,
    pub income: bool,
}

pub async fn init_table(conn: &mut PoolConnection<Sqlite>) -> Result<(), sqlx::Error> {
    let _ = sqlx::query(
        "CREATE TABLE categories (
            base_category TEXT NOT NULL,
            category      TEXT NOT NULL,
            income        INTEGER
        );",
    )
    .execute(&mut **conn)
    .await?;

    Ok(())
}

pub async fn insert(
    conn: &mut PoolConnection<Sqlite>,
    category: Category,
) -> Result<(), sqlx::Error> {
    let _ = sqlx::query(
        "INSERT INTO categories (base_category, category, income) values (?1, ?2, ?3);",
    )
    .bind(category.base_category)
    .bind(category.category)
    .bind(category.income)
    .execute(&mut **conn)
    .await?;

    Ok(())
}
