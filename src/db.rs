use color_eyre::Result;
use color_eyre::eyre::Context;
use sqlx::pool::{PoolConnection, PoolOptions};
use sqlx::postgres::PgConnectOptions;
use sqlx::{PgPool, Postgres};

use crate::config::{DatabaseConfig, IncomeType};
use crate::importer::Transaction;
use crate::importer::categorizer::CategorizationResult;

pub async fn build(config: &DatabaseConfig) -> Result<PgPool> {
    let options = PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .username(&config.username)
        .password(&config.password);

    let pool = PoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .wrap_err("Failed to open database")?;

    let mut conn = pool.acquire().await.wrap_err("Failed to get DB handle")?;

    // TODO: Are categories needed with grafana?
    sqlx::raw_sql(
        "
        DROP TABLE IF EXISTS accounts;
        DROP TABLE IF EXISTS categories;
        DROP TABLE IF EXISTS transactions;

        CREATE TABLE categories (
            id            serial PRIMARY KEY,
            base_category text NOT NULL,
            category      text NOT NULL,
            income        boolean
        );

        CREATE TABLE transactions (
            id               serial PRIMARY KEY,
            account          text NOT NULL,
            base_category    text NOT NULL,
            category         text NOT NULL,
            source_category  text,
            income           boolean,
            transaction_type text not null,
            posted_date      date,
            amount           NUMERIC(16, 2),
            transaction_id   text,
            name             text NOT NULL,
            memo             text
        );
        ",
    )
    .execute(&mut *conn)
    .await
    .wrap_err("Failed to setup database tables")?;

    Ok(pool)
}

pub struct DbConnection {
    pub conn: PoolConnection<Postgres>,
}

impl<'a> DbConnection {
    pub async fn add_category(&mut self, category: &str, income: bool) -> Result<()> {
        let base_category = category.split('.').next().unwrap();

        sqlx::query(
            "INSERT INTO categories (base_category, category, income) values ($1, $2, $3);",
        )
        .bind(base_category)
        .bind(category)
        .bind(income)
        .execute(&mut *self.conn)
        .await
        .wrap_err("Failed to add category")?;

        Ok(())
    }

    pub async fn add_transaction(
        &mut self,
        account: &str,
        categorization: CategorizationResult,
        transaction: Transaction<'a>,
    ) -> Result<()> {
        let base_category = categorization.category.split('.').next().unwrap();
        let income = match categorization.income {
            IncomeType::Yes => true,
            IncomeType::No => false,
            IncomeType::Auto => transaction.amount.is_sign_positive(),
        };

        sqlx::query(
            "INSERT INTO transactions (
                account,
                base_category,
                category,
                source_category,
                income,
                transaction_type,
                posted_date,
                amount,
                transaction_id,
                name,
                memo
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11
            );",
        )
        .bind(account)
        .bind(base_category)
        .bind(categorization.category)
        .bind(transaction.category)
        .bind(income)
        .bind(transaction.transaction_type.name())
        .bind(transaction.date_posted)
        .bind(transaction.amount)
        .bind(transaction.transaction_id)
        .bind(transaction.name)
        .bind(transaction.memo)
        .execute(&mut *self.conn)
        .await
        .wrap_err("Failed to add transaction")?;

        Ok(())
    }
}
