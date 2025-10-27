mod schema;

use std::path::PathBuf;

use chrono::NaiveDate;
use color_eyre::Result;
use color_eyre::eyre::Context;
use serde::Serialize;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::{SqlitePoolOptions, SqliteRow};
use sqlx::{Row, Sqlite, SqlitePool};
use tokio_stream::{Stream, StreamExt};

pub use self::schema::categories::Category;
pub use self::schema::transactions::{Transaction, TransactionType};
pub use self::schema::uncategorized_transactions::UncategorizedTransaction;

pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn build() -> Result<SqlitePool> {
        let pool = SqlitePoolOptions::new()
            .min_connections(2)
            .max_connections(8)
            .connect("sqlite::memory:")
            .await
            .wrap_err("Failed to open database")?;

        let mut conn = pool.acquire().await.wrap_err("Failed to get DB handle")?;

        schema::accounts::init_table(&mut conn)
            .await
            .wrap_err("Failed to create accounts table")?;

        schema::categories::init_table(&mut conn)
            .await
            .wrap_err("Failed to create categories table")?;

        schema::dates::init_table(&mut conn)
            .await
            .wrap_err("Failed to create dates table")?;

        schema::transactions::init_table(&mut conn)
            .await
            .wrap_err("Failed to create transactions table")?;

        schema::uncategorized_transactions::init_table(&mut conn)
            .await
            .wrap_err("Failed to create uncategorized_transactions table")?;

        Ok(pool)
    }

    pub async fn open_handle(&self) -> Result<DbHandle> {
        let conn = self.pool.acquire().await?;
        Ok(DbHandle { conn })
    }
}

pub struct DbHandle {
    conn: PoolConnection<Sqlite>,
}

impl<'a> DbHandle {
    pub async fn add_category(&mut self, category: Category) -> Result<()> {
        schema::categories::insert(&mut self.conn, category)
            .await
            .wrap_err("Failed to add category")?;

        Ok(())
    }

    pub async fn add_date(&mut self, date: NaiveDate) -> Result<()> {
        schema::dates::insert(&mut self.conn, date)
            .await
            .wrap_err("Failed to add date")?;

        Ok(())
    }

    pub async fn try_add_account(&mut self, name: &str) -> Result<()> {
        schema::accounts::insert(&mut self.conn, name)
            .await
            .wrap_err("Failed to add account")?;

        Ok(())
    }

    pub async fn add_transaction(&mut self, transaction: Transaction) -> Result<()> {
        schema::transactions::insert(&mut self.conn, transaction)
            .await
            .wrap_err("Failed to add transaction")
    }

    pub async fn add_uncategorized_transaction(
        &mut self,
        transaction: UncategorizedTransaction,
    ) -> Result<()> {
        schema::uncategorized_transactions::insert(&mut self.conn, transaction)
            .await
            .wrap_err("Failed to add uncategorized_transaction")
    }

    pub async fn get_transactions(&mut self) -> Result<Vec<Transaction>> {
        schema::transactions::list(&mut self.conn)
            .await
            .wrap_err("Failed to list transactions")
    }

    pub async fn get_expenses_over_time(&mut self) -> Result<TransactionsByCategory> {
        let mut rows = sqlx::query(
            "SELECT
                c2.base_category,
                d2.date_str,
                MAX(IFNULL(t2.amount, 0.0), 0.0) as amount
            FROM (
                SELECT DISTINCT
                    strftime('%Y-%m-01', d.date_str) as date_str
                FROM
                    dates d
                ORDER BY
                    date_str
            ) d2
            CROSS JOIN (
                SELECT DISTINCT 
                    c.base_category
                FROM
                    categories c
                WHERE
                    c.income = FALSE
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
                    t.income = FALSE
                GROUP BY
                    ds,
                    base_category
            ) t2 ON t2.base_category = c2.base_category
                AND t2.ds = d2.date_str;",
        )
        .fetch(&mut *self.conn);

        TransactionsByCategory::from_rows(&mut rows).await
    }

    pub async fn get_income_over_time(&mut self) -> Result<TransactionsByCategory> {
        let mut rows = sqlx::query(
            "SELECT
                c2.base_category,
                d2.date_str,
                MAX(IFNULL(t2.amount, 0.0), 0.0) as amount
            FROM (
                SELECT DISTINCT
                    strftime('%Y-%m-01', d.date_str) as date_str
                FROM
                    dates d
                ORDER BY
                    date_str
            ) d2
            CROSS JOIN (
                SELECT DISTINCT 
                    c.base_category
                FROM
                    categories c
                WHERE
                    c.income = TRUE
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
                    t.income = TRUE
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
                if !current_row.is_empty() {
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
