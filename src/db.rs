//! SQLite storage for imported transactions.
//!
//! The database is rebuilt from the source files on every start, so the schema is simply
//! dropped and recreated by [`Db::open`].

use std::path::Path;
use std::time::Duration;

use chrono::NaiveDate;
use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};
use rust_decimal::Decimal;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Row, Sqlite, SqlitePool};

use crate::config::{IncomeType, UserTransactionType};
use crate::importer::categorizer::{Categorization, UncategorizedTransaction};
use crate::importer::{Transaction, TransactionType};
use crate::model::{
    Dataset, TransactionRecord, UncategorizedKind, UncategorizedRecord, from_cents, to_cents,
};

/// Schema, applied to a fresh database.
const SCHEMA: &str = "
    DROP TABLE IF EXISTS loaded_files;
    DROP TABLE IF EXISTS transactions;
    DROP TABLE IF EXISTS uncategorized_transactions;
    DROP TABLE IF EXISTS budgets;

    CREATE TABLE loaded_files (
        id               INTEGER PRIMARY KEY,
        file_path        TEXT NOT NULL UNIQUE
    );

    CREATE TABLE transactions (
        id               INTEGER PRIMARY KEY,
        account          TEXT NOT NULL,
        base_category    TEXT NOT NULL,
        category         TEXT NOT NULL,
        source_category  TEXT,
        income           INTEGER NOT NULL,
        transaction_type TEXT NOT NULL,
        posted_date      TEXT NOT NULL,
        amount_cents     INTEGER NOT NULL,
        transaction_id   TEXT,
        name             TEXT NOT NULL,
        memo             TEXT
    );
    CREATE INDEX transactions_date ON transactions (posted_date);

    CREATE TABLE uncategorized_transactions (
        id           INTEGER PRIMARY KEY,
        missing_rule INTEGER NOT NULL,
        account      TEXT NOT NULL,
        type         TEXT NOT NULL,
        message      TEXT NOT NULL,
        posted_date  TEXT NOT NULL,
        amount_cents INTEGER NOT NULL
    );

    CREATE TABLE budgets (
        id           INTEGER PRIMARY KEY,
        name         TEXT NOT NULL,
        start_date   TEXT NOT NULL,
        end_date     TEXT,
        category     TEXT NOT NULL,
        amount_cents INTEGER NOT NULL
    );
";

/// Where the database lives.
#[derive(Debug, Clone, Copy)]
pub enum DbLocation<'a> {
    /// A file on disk. Any existing contents are discarded.
    File(&'a Path),
    /// A private in-memory database (used by tests).
    Memory,
}

/// A connection pool to the transaction database.
#[derive(Debug, Clone)]
pub struct Db {
    /// The underlying pool. Limited to a single connection: SQLite only supports one writer, so
    /// concurrent file imports queue on the pool instead of failing with `SQLITE_BUSY`.
    pool: SqlitePool,
}

impl Db {
    /// Open (creating if needed) the database and reset its schema.
    ///
    /// # Errors
    /// Fails if the database cannot be opened or the schema cannot be applied.
    pub async fn open(location: DbLocation<'_>) -> Result<Self> {
        let options = match location {
            DbLocation::File(path) => {
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .wrap_err("Failed to create database directory")?;
                }
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal)
            }
            DbLocation::Memory => SqliteConnectOptions::new().in_memory(true),
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            // Keep the single connection alive: closing it would discard an in-memory database.
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            // Imports wait on the pool while another file is being written.
            .acquire_timeout(Duration::from_secs(60 * 60))
            .connect_with(options)
            .await
            .wrap_err("Failed to open database")?;

        sqlx::raw_sql(SCHEMA)
            .execute(&pool)
            .await
            .wrap_err("Failed to setup database tables")?;

        Ok(Self { pool })
    }

    /// Begin a database transaction.
    ///
    /// # Errors
    /// Fails if a connection cannot be acquired.
    pub async fn start_transaction(&self) -> Result<DbTransaction<'static>> {
        let conn = self.pool.begin().await?;
        Ok(DbTransaction { conn })
    }

    /// Read all categorized transactions, ordered by date then id.
    ///
    /// # Errors
    /// Fails on a database error or corrupt row.
    pub async fn transactions(&self) -> Result<Vec<TransactionRecord>> {
        let rows = sqlx::query(
            "SELECT id, account, base_category, category, source_category, income,
                transaction_type, posted_date, amount_cents, transaction_id, name, memo
            FROM transactions ORDER BY posted_date, id",
        )
        .fetch_all(&self.pool)
        .await
        .wrap_err("Failed to read transactions")?;

        rows.into_iter()
            .map(|row| {
                Ok(TransactionRecord {
                    id: row.try_get("id")?,
                    account: row.try_get("account")?,
                    base_category: row.try_get("base_category")?,
                    category: row.try_get("category")?,
                    source_category: row.try_get("source_category")?,
                    income: row.try_get("income")?,
                    transaction_type: row.try_get("transaction_type")?,
                    posted_date: row.try_get("posted_date")?,
                    amount: from_cents(row.try_get("amount_cents")?),
                    transaction_id: row.try_get("transaction_id")?,
                    name: row.try_get("name")?,
                    memo: row.try_get("memo")?,
                })
            })
            .collect()
    }

    /// Read all uncategorized transactions, ordered by date then id.
    ///
    /// # Errors
    /// Fails on a database error or corrupt row.
    pub async fn uncategorized(&self) -> Result<Vec<UncategorizedRecord>> {
        let rows = sqlx::query(
            "SELECT id, missing_rule, account, type, message, posted_date, amount_cents
            FROM uncategorized_transactions ORDER BY posted_date, id",
        )
        .fetch_all(&self.pool)
        .await
        .wrap_err("Failed to read uncategorized transactions")?;

        rows.into_iter()
            .map(|row| {
                let missing_rule: bool = row.try_get("missing_rule")?;
                let type_name: String = row.try_get("type")?;
                let kind = if missing_rule {
                    UncategorizedKind::MissingRule(
                        UserTransactionType::from_name(&type_name)
                            .ok_or_else(|| eyre!("Unknown transaction type {type_name}"))?,
                    )
                } else {
                    UncategorizedKind::MissingType(
                        TransactionType::from_name(&type_name)
                            .ok_or_else(|| eyre!("Unknown source type {type_name}"))?,
                    )
                };
                Ok(UncategorizedRecord {
                    id: row.try_get("id")?,
                    kind,
                    account: row.try_get("account")?,
                    message: row.try_get("message")?,
                    posted_date: row.try_get("posted_date")?,
                    amount: from_cents(row.try_get("amount_cents")?),
                })
            })
            .collect()
    }

    /// Read everything needed by the GUI.
    ///
    /// # Errors
    /// Fails on a database error.
    pub async fn dataset(&self) -> Result<Dataset> {
        Ok(Dataset {
            transactions: self.transactions().await?,
            uncategorized: self.uncategorized().await?,
        })
    }
}

/// An open database transaction. Changes are discarded unless [`Self::commit`] is called.
pub struct DbTransaction<'a> {
    /// The underlying sqlx transaction.
    conn: sqlx::Transaction<'a, Sqlite>,
}

impl DbTransaction<'_> {
    /// Record that a file has been imported.
    ///
    /// # Errors
    /// Fails on a database error.
    pub async fn add_loaded_file(&mut self, file_name: &str) -> Result<()> {
        sqlx::query("INSERT INTO loaded_files (file_path) values ($1);")
            .bind(file_name)
            .execute(&mut *self.conn)
            .await?;

        Ok(())
    }

    /// Check whether a file has already been imported.
    ///
    /// # Errors
    /// Fails on a database error.
    pub async fn check_loaded_file(&mut self, file_name: &str) -> Result<bool> {
        let existing_file = sqlx::query("SELECT (id) FROM loaded_files WHERE file_path = $1;")
            .bind(file_name)
            .fetch_optional(&mut *self.conn)
            .await?;

        Ok(existing_file.is_some())
    }

    /// Store a transaction that could not be categorized.
    ///
    /// # Errors
    /// Fails on a database error.
    pub async fn add_uncategorized_transaction(
        &mut self,
        transaction: UncategorizedTransaction,
        posted_date: NaiveDate,
        amount: Decimal,
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
                missing_rule, account, type, message, posted_date, amount_cents
            ) values ($1, $2, $3, $4, $5, $6);",
        )
        .bind(missing_rule)
        .bind(account)
        .bind(missing_type)
        .bind(message)
        .bind(posted_date)
        .bind(to_cents(amount))
        .execute(&mut *self.conn)
        .await?;

        Ok(())
    }

    /// Store a categorized transaction.
    ///
    /// # Errors
    /// Fails on a database error.
    pub async fn add_transaction(
        &mut self,
        account: &str,
        categorization: Categorization<'_>,
        transaction: Transaction<'_>,
    ) -> Result<()> {
        let base_category = categorization
            .category
            .split('.')
            .next()
            .unwrap_or(categorization.category);
        let income = match categorization.income {
            IncomeType::Yes => true,
            IncomeType::No => false,
            IncomeType::Auto => transaction.amount.is_sign_positive(),
        };

        sqlx::query(
            "INSERT INTO transactions (
                account, base_category, category, source_category, income, transaction_type,
                posted_date, amount_cents, transaction_id, name, memo
            ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11);",
        )
        .bind(account)
        .bind(base_category)
        .bind(categorization.category)
        .bind(transaction.category.as_deref())
        .bind(income)
        .bind(categorization.transaction_type.name())
        .bind(transaction.date_posted)
        .bind(to_cents(transaction.amount))
        .bind(transaction.transaction_id.as_deref())
        .bind(transaction.name.as_ref())
        .bind(transaction.memo.as_deref())
        .execute(&mut *self.conn)
        .await
        .wrap_err("Failed to add transaction")?;

        Ok(())
    }

    /// Store one budget limit.
    ///
    /// # Errors
    /// Fails on a database error.
    pub async fn add_budget(
        &mut self,
        name: &str,
        start: NaiveDate,
        end: Option<NaiveDate>,
        category: &str,
        amount: Decimal,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO budgets (name, start_date, end_date, category, amount_cents)
            values ($1, $2, $3, $4, $5);",
        )
        .bind(name)
        .bind(start)
        .bind(end)
        .bind(category)
        .bind(to_cents(amount))
        .execute(&mut *self.conn)
        .await
        .wrap_err("Failed to add budget")?;

        Ok(())
    }

    /// Commit all changes made in this transaction.
    ///
    /// # Errors
    /// Fails if the commit fails.
    pub async fn commit(self) -> Result<()> {
        self.conn
            .commit()
            .await
            .wrap_err("Failed to commit database transaction")
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    fn date(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, m, d).unwrap()
    }

    #[tokio::test]
    async fn stores_and_reads_transactions() {
        let db = Db::open(DbLocation::Memory).await.unwrap();
        let mut tx = db.start_transaction().await.unwrap();
        for (income, amount) in [(IncomeType::No, -1250), (IncomeType::Auto, 300)] {
            tx.add_transaction(
                "Chequing",
                Categorization {
                    income,
                    ignore: false,
                    category: "Food.Groceries",
                    transaction_type: UserTransactionType::DebitPurchase,
                },
                Transaction {
                    transaction_type: TransactionType::Pos,
                    date_posted: date(2, 3),
                    amount: Decimal::new(amount, 2),
                    transaction_id: Some(Cow::Borrowed("1")),
                    category: None,
                    name: Cow::Borrowed("GROCER"),
                    memo: Some(Cow::Borrowed("memo")),
                },
            )
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();

        let rows = db.transactions().await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].base_category, "Food");
        assert_eq!(rows[0].category, "Food.Groceries");
        assert_eq!(rows[0].amount, Decimal::new(-1250, 2));
        assert!(!rows[0].income);
        assert!(rows[1].income, "auto income with positive amount");
        assert_eq!(rows[0].posted_date, date(2, 3));
        assert_eq!(rows[0].memo.as_deref(), Some("memo"));
        assert_eq!(rows[0].transaction_type, "DebitPurchase");
    }

    #[tokio::test]
    async fn stores_and_reads_uncategorized() {
        let db = Db::open(DbLocation::Memory).await.unwrap();
        let mut tx = db.start_transaction().await.unwrap();
        tx.add_uncategorized_transaction(
            UncategorizedTransaction::MissingType {
                account: "A".into(),
                source_type: TransactionType::Atm,
                name: "WITHDRAW".into(),
            },
            date(1, 1),
            Decimal::new(-2000, 2),
        )
        .await
        .unwrap();
        tx.add_uncategorized_transaction(
            UncategorizedTransaction::MissingRule {
                account: "A".into(),
                transaction_type: UserTransactionType::Interest,
                display: "INTEREST".into(),
            },
            date(1, 2),
            Decimal::new(5, 2),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let data = db.dataset().await.unwrap();
        assert!(data.transactions.is_empty());
        assert_eq!(data.uncategorized.len(), 2);
        assert_eq!(
            data.uncategorized[0].kind,
            UncategorizedKind::MissingType(TransactionType::Atm)
        );
        assert_eq!(
            data.uncategorized[1].kind,
            UncategorizedKind::MissingRule(UserTransactionType::Interest)
        );
        assert_eq!(data.uncategorized[1].amount, Decimal::new(5, 2));
    }

    #[tokio::test]
    async fn rollback_without_commit() {
        let db = Db::open(DbLocation::Memory).await.unwrap();
        {
            let mut tx = db.start_transaction().await.unwrap();
            tx.add_loaded_file("a.qfx").await.unwrap();
            assert!(tx.check_loaded_file("a.qfx").await.unwrap());
        }
        let mut tx = db.start_transaction().await.unwrap();
        assert!(!tx.check_loaded_file("a.qfx").await.unwrap());
        tx.add_budget("B", date(1, 1), Some(date(2, 1)), "Food", Decimal::ONE)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    #[tokio::test]
    async fn file_database_is_reset_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("money.sqlite");
        let db = Db::open(DbLocation::File(&path)).await.unwrap();
        let mut tx = db.start_transaction().await.unwrap();
        tx.add_loaded_file("a.qfx").await.unwrap();
        tx.commit().await.unwrap();
        drop(db);

        let db = Db::open(DbLocation::File(&path)).await.unwrap();
        let mut tx = db.start_transaction().await.unwrap();
        assert!(!tx.check_loaded_file("a.qfx").await.unwrap());
    }
}
