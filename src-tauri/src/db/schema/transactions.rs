use chrono::NaiveDate;
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use sqlx::pool::PoolConnection;
use sqlx::{Row, Sqlite};
use tokio_stream::StreamExt;

#[derive(
    Debug, IntoPrimitive, FromPrimitive, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Serialize,
)]
#[repr(u8)]
pub enum TransactionType {
    Debit,
    Credit,
    Pos,
    Atm,
    Fee,
    #[num_enum(default)]
    Other,
}

#[derive(Debug, Serialize)]
pub struct Transaction {
    pub account: String,
    pub base_category: String,
    pub category: String,
    pub source_category: Option<String>,
    pub income: bool,
    pub transaction_type: TransactionType,
    pub date: NaiveDate,
    pub amount: f64,
    pub transaction_id: Option<String>,
    pub name: String,
    pub memo: Option<String>,
}

pub async fn init_table(conn: &mut PoolConnection<Sqlite>) -> Result<(), sqlx::Error> {
    let _ = sqlx::query(
        "CREATE TABLE transactions (
            id               INTEGER PRIMARY KEY,
            account          TEXT NOT NULL,
            base_category    TEXT NOT NULL,
            category         TEXT NOT NULL,
            source_category  TEXT,
            income           INTEGER,
            transaction_type INTEGER,
            date_str         TEXT,
            amount           REAL,
            transaction_id   TEXT,
            name             TEXT NOT NULL,
            memo             TEXT
        );",
    )
    .execute(&mut **conn)
    .await?;

    Ok(())
}

pub async fn insert(
    conn: &mut PoolConnection<Sqlite>,
    transaction: Transaction,
) -> Result<(), sqlx::Error> {
    let _ = sqlx::query(
        "INSERT INTO transactions (
                account,
                base_category,
                category,
                source_category,
                income,
                transaction_type,
                date_str,
                amount,
                transaction_id,
                name,
                memo
            ) values (
                ?1,
                ?2,
                ?3,
                ?4,
                ?5,
                ?6,
                ?7,
                ?8,
                ?9,
                ?10,
                ?11
            );",
    )
    .bind(transaction.account)
    .bind(transaction.base_category)
    .bind(transaction.category)
    .bind(transaction.source_category)
    .bind(transaction.income)
    .bind::<u8>(transaction.transaction_type.into())
    .bind(transaction.date)
    .bind(transaction.amount)
    .bind(transaction.transaction_id)
    .bind(transaction.name)
    .bind(transaction.memo)
    .execute(&mut **conn)
    .await?;

    Ok(())
}

pub async fn list(conn: &mut PoolConnection<Sqlite>) -> Result<Vec<Transaction>, sqlx::Error> {
    let mut rows = sqlx::query(
        "SELECT
                t.account,
                t.base_category,
                t.category,
                t.source_category,
                t.income,
                t.transaction_type,
                t.date_str,
                t.amount,
                t.transaction_id,
                t.name,
                t.memo
            FROM
                transactions t;",
    )
    .fetch(&mut **conn);

    let mut transactions = Vec::new();
    while let Some(row) = rows.try_next().await? {
        transactions.push(Transaction {
            account: row.try_get(0usize)?,
            base_category: row.try_get(1usize)?,
            category: row.try_get(2usize)?,
            source_category: row.try_get(3usize)?,
            income: row.try_get(4usize)?,
            transaction_type: TransactionType::from_primitive(row.try_get::<u8, usize>(5usize)?),
            date: row.try_get(6usize)?,
            amount: row.try_get(7usize)?,
            transaction_id: row.try_get(8usize)?,
            name: row.try_get(9usize)?,
            memo: row.try_get(10usize)?,
        });
    }

    Ok(transactions)
}
