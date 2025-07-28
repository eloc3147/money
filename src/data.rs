use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use color_eyre::Result;
use num_enum::IntoPrimitive;
use rusqlite::backup::Backup;
use rusqlite::{Connection, Statement};

/// Transaction type as defined by the input file
#[derive(Debug, IntoPrimitive)]
#[repr(u8)]
pub enum FileTransactionType {
    Debit,
    Credit,
    Pos,
    Atm,
    Fee,
    Other,
}

#[derive(Debug)]
pub struct FileTransaction<'a> {
    pub transaction_type: FileTransactionType,
    pub date_posted: DateTime<FixedOffset>,
    pub amount: f64,
    pub transaction_id: &'a str,
    pub name: &'a str,
    pub memo: Option<&'a str>,
}

pub struct DataStore {
    conn: Connection,
}

impl<'a> DataStore {
    pub fn new() -> Result<Self> {
        let conn = Connection::open_in_memory()?;

        conn.execute(
            "CREATE TABLE accounts (
                id   INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            )",
            (),
        )?;
        // Can account be linked?
        conn.execute(
            "CREATE TABLE transactions (
                id               INTEGER PRIMARY KEY,
                account          INTEGER,
                category         TEXT NOT NULL,
                transaction_type INTEGER,
                date_posted      INTEGER,
                amount           REAL,
                transaction_id   INTEGER,
                name             TEXT NOT NULL,
                memo             TEXT
            )",
            (),
        )?;

        Ok(Self { conn })
    }

    pub fn add_account(&mut self, name: &'a str) -> Result<u64> {
        let transaction = self.conn.transaction()?;
        transaction.execute("INSERT INTO accounts (name) values (?);", [name])?;
        let id: u64 = transaction.query_one("SELECT LAST_INSERT_ROWID();", (), |row| row.get(0))?;

        transaction.commit()?;
        Ok(id)
    }

    pub fn build_batch_insert_handle(&'a self) -> Result<BatchInsertHandle<'a>> {
        let add_transaction = self.conn.prepare(
        "INSERT INTO transactions (account, category, transaction_type, date_posted, amount, transaction_id, name, memo) values (?, ?, ?, ?, ?, ?, ?, ?);"
        )?;

        Ok(BatchInsertHandle { add_transaction })
    }

    pub fn dump_to_file(&self, out_file: &Path) -> Result<()> {
        let mut dest = Connection::open(out_file)?;
        let backup = Backup::new(&self.conn, &mut dest)?;
        backup.run_to_completion(32, Duration::ZERO, None)?;
        Ok(())
    }
}

pub struct BatchInsertHandle<'a> {
    add_transaction: Statement<'a>,
}

impl BatchInsertHandle<'_> {
    pub fn add_transaction(
        &mut self,
        account_id: u64,
        category: &str,
        transaction_type: FileTransactionType,
        date_posted: DateTime<FixedOffset>,
        amount: f64,
        transaction_id: &str,
        name: &str,
        memo: Option<&str>,
    ) -> Result<()> {
        let transaction_type: u8 = transaction_type.into();

        let _ = self.add_transaction.execute((
            account_id,
            category,
            transaction_type,
            date_posted,
            amount,
            transaction_id,
            name,
            memo,
        ))?;
        Ok(())
    }
}
