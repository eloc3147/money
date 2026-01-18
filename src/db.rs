use color_eyre::Result;
use color_eyre::eyre::Context;
use sqlx::pool::{PoolConnection, PoolOptions};
use sqlx::postgres::PgConnectOptions;
use sqlx::{PgPool, Postgres};

use crate::config::{DatabaseConfig, IncomeType};
use crate::importer::Transaction;
use crate::importer::categorizer::{Categorization, UncategorizedTransaction};

pub async fn build(config: &DatabaseConfig, clean: bool) -> Result<Db> {
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

    if clean {
        sqlx::raw_sql(
            "
            DROP TABLE IF EXISTS loaded_files;
            DROP TABLE IF EXISTS transactions;
            DROP TABLE IF EXISTS uncategorized_transactions;
            ",
        )
        .execute(&mut *conn)
        .await
        .wrap_err("Failed to setup database tables")?;
    }

    sqlx::raw_sql(
        "
        CREATE TABLE IF NOT EXISTS loaded_files (
            id               serial PRIMARY KEY,
            file_path        text NOT NULL
        );

        CREATE TABLE IF NOT EXISTS transactions (
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

        CREATE TABLE IF NOT EXISTS uncategorized_transactions (
            id           serial PRIMARY KEY,
            missing_rule boolean,
            account      text NOT NULL,
            type         text NOT NULL,
            message      text NOT NULL
        );
        ",
    )
    .execute(&mut *conn)
    .await
    .wrap_err("Failed to setup database tables")?;

    Ok(Db { pool })
}

pub struct Db {
    pool: PgPool,
}

impl Db {
    pub async fn open_handle(&self) -> Result<DbHandle> {
        let conn = self.pool.acquire().await?;
        Ok(DbHandle { conn })
    }
}

pub struct DbHandle {
    conn: PoolConnection<Postgres>,
}

impl DbHandle {
    pub async fn add_loaded_file(&mut self, file_name: &str) -> Result<()> {
        sqlx::query("INSERT INTO loaded_files (file_path) values ($1);")
            .bind(file_name)
            .execute(&mut *self.conn)
            .await?;

        Ok(())
    }

    pub async fn check_loaded_file(&mut self, file_name: &str) -> Result<bool> {
        let existing_file = sqlx::query("SELECT (id) FROM loaded_files WHERE file_path = $1;")
            .bind(file_name)
            .fetch_optional(&mut *self.conn)
            .await?;

        Ok(existing_file.is_some())
    }

    pub async fn add_uncategorized_transaction(
        &mut self,
        transaction: UncategorizedTransaction,
    ) -> Result<()> {
        let (missing_rule, account, missing_type, message) = match transaction {
            UncategorizedTransaction::MissingType {
                account,
                source_type,
                name,
            } => (false, account, source_type.name(), name),
            UncategorizedTransaction::MissingRule {
                account,
                transaction_type,
                display,
            } => (true, account, transaction_type.name(), display),
        };

        sqlx::query(
            "INSERT INTO uncategorized_transactions (
                missing_rule,
                account,
                type,
                message
            ) values (
                $1,
                $2,
                $3,
                $4
            );",
        )
        .bind(missing_rule)
        .bind(account)
        .bind(missing_type)
        .bind(message)
        .execute(&mut *self.conn)
        .await?;

        Ok(())
    }

    pub async fn add_transaction<'t>(
        &'t mut self,
        account: &str,
        categorization: Categorization,
        transaction: Transaction<'t>,
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
