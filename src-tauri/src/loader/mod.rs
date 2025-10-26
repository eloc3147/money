pub mod config;

pub mod categorizer;
mod csv_file;
mod qfx_file;

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use categorizer::Categorizer;
use chrono::{Days, NaiveDate};
use color_eyre::eyre::{Context, Result, eyre};
use config::AccountConfig;
use futures::future::try_join;
use num_enum::{FromPrimitive, IntoPrimitive};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::LoadState;
use crate::db::DbConnection;
use crate::loader::config::AppConfig;
use crate::loader::csv_file::{CsvReader, CsvTransactionIter};
use crate::loader::qfx_file::{QfxReader, QfxTransactionIter};

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

#[derive(Debug)]
pub struct ImportTransaction<'a> {
    pub transaction_type: TransactionType,
    pub date_posted: NaiveDate,
    pub amount: f64,
    pub transaction_id: Option<Cow<'a, str>>,
    pub category: Option<Cow<'a, str>>,
    pub name: Cow<'a, str>,
    pub memo: Option<Cow<'a, str>>,
}

pub enum TransactionReader {
    QfxReader(QfxReader),
    CsvReader(CsvReader),
}

impl<'a> TransactionReader {
    pub fn transactions(&'a mut self) -> Result<TransactionIter<'a>> {
        match self {
            Self::QfxReader(r) => Ok(TransactionIter::QfxIter(
                r.read().wrap_err("Failed to read from QfxReader")?,
            )),
            Self::CsvReader(r) => Ok(TransactionIter::CsvIter(
                r.read().wrap_err("Failed to read from CsvReader")?,
            )),
        }
    }
}

pub enum TransactionIter<'a> {
    QfxIter(QfxTransactionIter<'a>),
    CsvIter(CsvTransactionIter<'a>),
}

impl<'a> Iterator for TransactionIter<'a> {
    type Item = Result<ImportTransaction<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::QfxIter(i) => i.next(),
            Self::CsvIter(i) => i.next(),
        }
    }
}

async fn list_accounts(
    accounts: &[AccountConfig],
    total_count: &AtomicUsize,
    file_queue: Sender<(String, PathBuf)>,
) -> Result<()> {
    let mut stack = Vec::new();
    for account in accounts {
        stack.push(account.source_path.clone());

        while let Some(dir) = stack.pop() {
            let mut read_dir = tokio::fs::read_dir(dir).await?;
            while let Some(entry) = read_dir.next_entry().await? {
                let entry_type = entry.file_type().await?;
                if entry_type.is_dir() {
                    stack.push(entry.path());
                } else if entry_type.is_file() {
                    let _ = total_count.fetch_add(1, Ordering::SeqCst);
                    file_queue
                        .send((account.name.clone(), entry.path()))
                        .await?;
                } else if entry_type.is_symlink() {
                    let new_path = tokio::fs::read_link(entry.path()).await?;
                    let new_meta = tokio::fs::metadata(&new_path).await?;

                    if new_meta.is_file() {
                        let _ = total_count.fetch_add(1, Ordering::SeqCst);
                        file_queue.send((account.name.clone(), new_path)).await?;
                    } else if new_meta.is_dir() {
                        stack.push(entry.path());
                    }
                }
            }
        }
    }

    Ok(())
}

async fn import_files(
    conn: &mut DbConnection,
    categorizer: &mut Categorizer,
    state: &Mutex<LoadState>,
    total_count: &AtomicUsize,
    mut file_queue: Receiver<(String, PathBuf)>,
) -> Result<()> {
    // Add transactions
    let mut first_date = NaiveDate::MAX;
    let mut last_date = NaiveDate::MIN;
    let mut load_count = 0;
    let mut account_ids = HashMap::new();

    while let Some((account_name, file)) = file_queue.recv().await {
        let mut lock = state.lock().expect("lock mutex");
        *lock = LoadState::LoadingFiles {
            account: account_name.clone(),
            done: load_count,
            total: total_count.load(Ordering::Relaxed),
        };
        drop(lock);

        let account_id = match account_ids.entry(account_name.clone()) {
            Entry::Vacant(e) => {
                let id = conn
                    .add_account(e.key().clone())
                    .await
                    .wrap_err("Failed to add account")?;
                e.insert(id);
                id
            }
            Entry::Occupied(e) => *e.get(),
        };

        let ext = file
            .extension()
            .ok_or_else(|| eyre!("File missing extension: {:?}", file))?
            .to_ascii_lowercase();

        let mut reader = match &*ext.to_string_lossy() {
            "qfx" => {
                let reader = QfxReader::open(&file)
                    .wrap_err_with(|| format!("Failed to read file: {}", file.to_string_lossy()))?;

                TransactionReader::QfxReader(reader)
            }
            "csv" => {
                let reader = CsvReader::open(&file)
                    .wrap_err_with(|| format!("Failed to read file: {}", file.to_string_lossy()))?;

                TransactionReader::CsvReader(reader)
            }
            ext => return Err(eyre!("Unrecognized file type: {}", ext)),
        };

        let transactions = reader.transactions()?;
        for transaction in transactions {
            let transaction = transaction?;

            if let Some(tid) = transaction.transaction_id.as_ref()
                && tid.contains(".")
                && transaction.amount == 0.0
            {
                // Weird multiline transaction. Extra lines don't contain much useful information
                continue;
            }

            let categorization_result = categorizer.categorize(
                &account_name,
                &transaction.name,
                transaction.transaction_type,
                transaction.memo.as_ref().map(|m| m.as_ref()),
            )?;
            let Some(categorization) = categorization_result else {
                continue;
            };

            if categorization.ignore {
                continue;
            }

            if transaction.date_posted < first_date {
                first_date = transaction.date_posted;
            }

            if transaction.date_posted > last_date {
                last_date = transaction.date_posted;
            }

            conn.add_transaction(account_id, categorization, transaction)
                .await?;
        }

        load_count = load_count.saturating_add(1);
    }

    let mut lock = state.lock().expect("lock mutex");
    *lock = LoadState::LoadingFiles {
        account: String::new(),
        done: load_count,
        total: total_count.load(Ordering::Relaxed),
    };
    drop(lock);

    // Add categories
    for (category, income) in categorizer.categories() {
        conn.add_category(category, *income).await?;
    }

    // Fill date range
    let mut add_date = first_date;
    while add_date <= last_date {
        conn.add_date(add_date).await?;
        add_date = add_date + Days::new(1);
    }

    Ok(())
}

pub async fn load(
    conn: &mut DbConnection,
    state: &Mutex<LoadState>,
    config_path: &Path,
) -> Result<()> {
    // Load config
    let mut lock = state.lock().expect("lock mutex");
    *lock = LoadState::LoadingConfig;
    drop(lock);

    let config = AppConfig::load(config_path)
        .await
        .map(|c| Box::leak(Box::new(c)))?;

    // Build rules
    let mut lock = state.lock().expect("lock mutex");
    *lock = LoadState::BuildingRules;
    drop(lock);

    let mut categorizer = Categorizer::build(&config.transaction_type, &config.rule)
        .wrap_err("Failed to load transaction rules")?;

    // Load transactions
    let total_count = AtomicUsize::new(0);
    let (file_tx, file_rx) = tokio::sync::mpsc::channel(10);
    try_join(
        list_accounts(&config.account, &total_count, file_tx),
        import_files(conn, &mut categorizer, state, &total_count, file_rx),
    )
    .await?;

    // Done
    let mut lock = state.lock().expect("lock mutex");
    *lock = LoadState::Done;
    drop(lock);

    Ok(())
}
