use sqlx::Sqlite;
use sqlx::pool::PoolConnection;

use crate::db::TransactionType;
use crate::loader::config::UserTransactionType;

#[derive(Debug)]
pub enum UncategorizedTransaction {
    MissingType {
        account: String,
        source_type: TransactionType,
        name: String,
    },
    MissingRule {
        account: String,
        transaction_type: UserTransactionType,
        display: String,
    },
}

pub async fn init_table(conn: &mut PoolConnection<Sqlite>) -> Result<(), sqlx::Error> {
    let _ = sqlx::query(
        "CREATE TABLE uncategorized_transactions (
            id           INTEGER PRIMARY KEY,
            missing_rule INTEGER,
            account      TEXT NOT NULL,
            type         INTEGER
            text         TEXT NOT NULL
        );",
    )
    .execute(&mut **conn)
    .await?;

    Ok(())
}

pub async fn insert(
    conn: &mut PoolConnection<Sqlite>,
    transaction: UncategorizedTransaction,
) -> Result<(), sqlx::Error> {
    let (missing_rule, account, missing_type, text) = match transaction {
        UncategorizedTransaction::MissingType {
            account,
            source_type,
            name,
        } => (0, account, source_type.into(), name),
        UncategorizedTransaction::MissingRule {
            account,
            transaction_type,
            display,
        } => (1, account, transaction_type.into(), display),
    };

    let _ = sqlx::query(
        "INSERT INTO uncategorized_transactions (
            missing_rule,
            account,
            type,
            text
        ) values (
            ?1,
            ?2,
            ?3,
            ?4
        );",
    )
    .bind(missing_rule)
    .bind(account)
    .bind::<u8>(missing_type)
    .bind(text)
    .execute(&mut **conn)
    .await?;

    Ok(())
}
