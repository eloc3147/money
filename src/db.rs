use std::fs::File;
use std::path::PathBuf;

use axum::extract::{FromRef, FromRequestParts};
use axum::http::StatusCode;
use axum::http::request::Parts;
use chrono::NaiveDate;
use color_eyre::Result;
use color_eyre::eyre::Context;
use futures::{Stream, TryStreamExt};
use serde::Serialize;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::{SqlitePoolOptions, SqliteRow};
use sqlx::{Connection, Row, Sqlite, SqlitePool};

use crate::importer::TransactionType;
use crate::server::internal_error;

pub async fn build() -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect("sqlite::memory:")
        .await
        .wrap_err("Failed to open database")?;

    let mut conn = pool.acquire().await.wrap_err("Failed to get DB handle")?;

    sqlx::query(
        "CREATE TABLE accounts (
            id   INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );",
    )
    .execute(&mut *conn)
    .await
    .wrap_err("Failed to create accounts table")?;

    sqlx::query(
        "CREATE TABLE categories (
            base_category TEXT NOT NULL,
            category TEXT NOT NULL
        );",
    )
    .execute(&mut *conn)
    .await
    .wrap_err("Failed to create categories table")?;

    sqlx::query(
        "CREATE TABLE dates (
            date_str TEXT
        );",
    )
    .execute(&mut *conn)
    .await
    .wrap_err("Failed to create dates table")?;

    sqlx::query(
        "CREATE TABLE transactions (
            id               INTEGER PRIMARY KEY,
            account          INTEGER,
            base_category    TEXT NOT NULL,
            category         TEXT NOT NULL,
            transaction_type INTEGER,
            date_str         TEXT,
            amount           REAL,
            transaction_id   INTEGER,
            name             TEXT NOT NULL,
            memo             TEXT
        );",
    )
    .execute(&mut *conn)
    .await
    .wrap_err("Failed to create transactions table")?;

    Ok(pool)
}

pub struct DbConnection {
    pub conn: PoolConnection<Sqlite>,
}

impl DbConnection {
    pub async fn add_category(&mut self, category: &str) -> Result<()> {
        let base_category = category.split(".").next().unwrap();

        sqlx::query("INSERT INTO categories (base_category, category) values (?1, ?2);")
            .bind(base_category)
            .bind(category)
            .execute(&mut *self.conn)
            .await
            .wrap_err("Failed to add category")?;

        Ok(())
    }

    pub async fn add_date(&mut self, date: NaiveDate) -> Result<()> {
        sqlx::query("INSERT INTO dates (date_str) values (?);")
            .bind(date.to_string())
            .execute(&mut *self.conn)
            .await
            .wrap_err("Failed to add date")?;

        Ok(())
    }

    pub async fn add_account(&mut self, account: String) -> Result<i64> {
        let account_id: i64 = self
            .conn
            .transaction(|txn| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO accounts (name) values (?1);")
                        .bind(account)
                        .execute(&mut **txn)
                        .await?;

                    sqlx::query_scalar("SELECT LAST_INSERT_ROWID();")
                        .fetch_one(&mut **txn)
                        .await
                })
            })
            .await
            .wrap_err("Failed to add categories")?;

        Ok(account_id)
    }

    pub async fn add_transaction(
        &mut self,
        account_id: i64,
        category: &str,
        transaction_type: TransactionType,
        date_posted: NaiveDate,
        amount: f64,
        transaction_id: &str,
        name: &str,
        memo: Option<&str>,
    ) -> Result<()> {
        let base_category = category.split(".").next().unwrap();

        sqlx::query(
            "INSERT INTO transactions (
                account,
                base_category,
                category,
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
                ?9
            );",
        )
        .bind(account_id)
        .bind(base_category)
        .bind(category)
        .bind::<u8>(transaction_type.into())
        .bind(date_posted.to_string())
        .bind(amount)
        .bind(transaction_id)
        .bind(name)
        .bind(memo)
        .execute(&mut *self.conn)
        .await
        .wrap_err("Failed to add transaction")?;

        Ok(())
    }

    pub async fn get_expense_transactions(&mut self) -> Result<TransactionsByCategory> {
        let mut rows = sqlx::query(
            "SELECT
                c2.base_category,
                d2.date_str,
                ABS(IFNULL(t2.amount, 0.0)) as amount
            FROM (
                SELECT DISTINCT
                    strftime('%Y-%m-01', d.date_str) as date_str
                FROM dates d
                ORDER BY date_str
            ) d2
            CROSS JOIN (
                SELECT DISTINCT 
                    c.base_category
                FROM
                    categories c
                ORDER BY
                    c.base_category
            ) c2
            LEFT JOIN (
                SELECT
                    t.base_category,
                    strftime('%Y-%m-01', t.date_str) as ds,
                    SUM(-t.amount) as amount
                FROM
                    transactions t
                WHERE
                    t.amount < 0
                GROUP BY
                    ds,
                    base_category
            ) t2 ON t2.base_category = c2.base_category
                AND t2.ds = d2.date_str;",
        )
        .fetch(&mut *self.conn);

        TransactionsByCategory::from_rows(&mut rows).await
    }

    pub async fn get_income_transactions(&mut self) -> Result<TransactionsByCategory> {
        let mut rows = sqlx::query(
            "SELECT
                c2.base_category,
                d2.date_str,
                ABS(IFNULL(t2.amount, 0.0)) as amount
            FROM (
                SELECT DISTINCT
                    strftime('%Y-%m-01', d.date_str) as date_str
                FROM dates d
                ORDER BY date_str
            ) d2
            CROSS JOIN (
                SELECT DISTINCT 
                    c.base_category
                FROM
                    categories c
                ORDER BY
                    c.base_category
            ) c2
            LEFT JOIN (
                SELECT
                    t.base_category,
                    strftime('%Y-%m-01', t.date_str) as ds,
                    SUM(t.amount) as amount
                FROM
                    transactions t
                WHERE
                    t.amount > 0
                GROUP BY
                    ds,
                    base_category
            ) t2 ON t2.base_category = c2.base_category
                AND t2.ds = d2.date_str;",
        )
        .fetch(&mut *self.conn);

        TransactionsByCategory::from_rows(&mut rows).await
    }

    pub async fn dump_transactions(&mut self) -> Result<()> {
        let dump_path = PathBuf::from("dmp.sqlite");
        if dump_path.exists() {
            std::fs::remove_file(dump_path)?;
        }

        sqlx::raw_sql("VACUUM INTO \"file:dmp.sqlite?mode=rwc\";")
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }
}

impl<S> FromRequestParts<S> for DbConnection
where
    SqlitePool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pool = SqlitePool::from_ref(state);

        let conn = pool.acquire().await.map_err(internal_error)?;

        Ok(Self { conn })
    }
}

#[derive(Debug, Serialize)]
pub struct TransactionsByCategory {
    categories: Vec<String>,
    dates: Vec<String>,
    amounts: Vec<Vec<f32>>,
}

impl TransactionsByCategory {
    async fn from_rows<F>(rows: &mut F) -> Result<Self>
    where
        F: Stream<Item = std::result::Result<SqliteRow, sqlx::Error>> + Unpin + Send,
    {
        let mut first_date = true;

        // Build hashmap of category to date to amount
        // This allows us to fill in missing dates, as well as separates transactions by category
        let mut row_len = 0;
        let mut column = 0;
        let mut current_row = Vec::new();
        let mut current_date = String::new();

        let mut categories = Vec::new();
        let mut dates = Vec::new();
        let mut amounts = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let category: &str = row.try_get(0usize)?;
            let date_str: &str = row.try_get(1usize)?;
            let amount: f32 = row.try_get(2usize)?;

            if current_date != date_str {
                if current_row.len() > 0 {
                    if first_date {
                        row_len = current_row.len();
                        first_date = false;
                    } else {
                        debug_assert_eq!(row_len, current_row.len());
                    }

                    amounts.push(current_row);
                    current_row = Vec::with_capacity(row_len);
                    column = 0;

                    dates.push(date_str.to_string());
                }

                current_date = date_str.to_string();
            }

            if first_date {
                categories.push(category.to_string());
            } else {
                debug_assert_eq!(category, categories[column]);
            }

            current_row.push(amount);

            column += 1;
        }

        Ok(Self {
            categories,
            dates,
            amounts,
        })
    }
}
