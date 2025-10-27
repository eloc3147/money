pub mod config;

pub mod categorizer;
mod csv_file;
mod qfx_file;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use categorizer::Categorizer;
use chrono::NaiveDate;
use color_eyre::eyre::{Context, Result, eyre};
use config::AccountConfig;
use futures::{StreamExt, TryStreamExt};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;

use crate::db::{Category, Db, Transaction, TransactionType};
use crate::loader::categorizer::CategorizationStatus;
use crate::loader::config::AppConfig;
use crate::loader::csv_file::CsvReader;
use crate::loader::qfx_file::QfxReader;
use crate::{LoadState, LoadStep};

#[derive(Debug)]
pub struct FileTransaction<'a> {
    pub transaction_type: TransactionType,
    pub date_posted: NaiveDate,
    pub amount: f64,
    pub transaction_id: Option<Cow<'a, str>>,
    pub category: Option<Cow<'a, str>>,
    pub name: Cow<'a, str>,
    pub memo: Option<Cow<'a, str>>,
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

struct ImportConfig<'a> {
    db: &'a Db,
    categorizer: &'a Categorizer,
    account_name: String,
    file_path: PathBuf,
}

async fn import_file(config: ImportConfig<'_>) -> Result<(NaiveDate, NaiveDate)> {
    let mut first_date = NaiveDate::MAX;
    let mut last_date = NaiveDate::MIN;

    let ext = config
        .file_path
        .extension()
        .ok_or_else(|| eyre!("File missing extension: {:?}", config.file_path))?
        .to_ascii_lowercase();

    let mut transactions = match &*ext.to_string_lossy() {
        "qfx" => {
            let reader = QfxReader::open(&config.file_path).wrap_err_with(|| {
                format!(
                    "Failed to open file: {}",
                    config.file_path.to_string_lossy()
                )
            })?;

            tokio_stream::iter(reader.read().wrap_err("Failed to read transactions")?).boxed()
        }
        "csv" => CsvReader::open(&config.file_path)
            .await
            .wrap_err_with(|| {
                format!(
                    "Failed to open file: {}",
                    config.file_path.to_string_lossy()
                )
            })?
            .read()
            .boxed(),
        ext => return Err(eyre!("Unrecognized file type: {}", ext)),
    };

    let mut conn = config.db.open_handle().await?;
    conn.try_add_account(&config.account_name).await?;

    while let Some(transaction) = transactions.try_next().await? {
        if let Some(tid) = transaction.transaction_id.as_ref()
            && tid.contains(".")
            && transaction.amount == 0.0
        {
            // Weird multiline transaction. Extra lines don't contain much useful information
            continue;
        }

        let categorization_result = config.categorizer.categorize(
            &config.account_name,
            &transaction.name,
            transaction.transaction_type,
            transaction.memo.as_ref().map(|m| m.as_ref()),
        )?;
        let categorization = match categorization_result {
            CategorizationStatus::Categorized(c) => c,
            CategorizationStatus::Uncategorized(t) => {
                conn.add_uncategorized_transaction(t).await?;
                continue;
            }
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

        conn.add_transaction(Transaction {
            account: config.account_name.clone(),
            base_category: categorization
                .category
                .split('.')
                .next()
                .expect("split to return at least 1 element")
                .to_string(),
            category: categorization.category,
            source_category: transaction.category.map(Cow::into_owned),
            income: categorization.income,
            transaction_type: transaction.transaction_type,
            date: transaction.date_posted,
            amount: transaction.amount,
            transaction_id: transaction.transaction_id.map(Cow::into_owned),
            name: transaction.name.into_owned(),
            memo: transaction.memo.map(Cow::into_owned),
        })
        .await?;
    }

    Ok((first_date, last_date))
}

async fn import_files(
    db: &Db,
    categorizer: &Categorizer,
    load_count: &AtomicUsize,
    file_queue: Receiver<(String, PathBuf)>,
) -> Result<(NaiveDate, NaiveDate)> {
    let dates = Arc::new(Mutex::new((NaiveDate::MAX, NaiveDate::MIN)));

    ReceiverStream::new(file_queue)
        .map(|(account_name, file_path)| {
            // Funky stuff to get all required state to the concurrent function
            let config = ImportConfig {
                db,
                categorizer,
                account_name,
                file_path,
            };
            let dates_ref = dates.clone();

            Ok((config, dates_ref))
        })
        .try_for_each_concurrent(8, async |(config, dates_ref)| -> Result<()> {
            let (fd, ld) = import_file(config).await?;

            let mut lock = dates_ref.lock().expect("mutex to unlock");
            lock.0 = std::cmp::min(lock.0, fd);
            lock.1 = std::cmp::max(lock.1, ld);

            load_count.fetch_add(1, Ordering::Relaxed);

            Ok(())
        })
        .await?;

    let final_dates = dates.lock().expect("mutex to unlock").clone();
    Ok(final_dates)
}

pub async fn load(db: &Db, state: &LoadState, config_path: &Path) -> Result<()> {
    // Load config
    {
        *state.step.lock().expect("lock mutex") = LoadStep::LoadingConfig;
    }

    let config = AppConfig::load(config_path).await?;

    // Build rules
    {
        *state.step.lock().expect("lock mutex") = LoadStep::BuildingRules;
    }

    let categorizer = Categorizer::build(config.transaction_type, config.rule)
        .wrap_err("Failed to load transaction rules")?;

    {
        *state.step.lock().expect("lock mutex") = LoadStep::LoadingFiles;
    }

    // Load transactions concurrently
    let (file_tx, file_rx) = tokio::sync::mpsc::channel(8);
    let (_, (first_date, last_date)) = futures::future::try_join(
        list_accounts(&config.account, &state.total_count, file_tx),
        import_files(db, &categorizer, &state.loaded_count, file_rx),
    )
    .await?;

    let mut conn = db.open_handle().await?;

    // Add categories
    for (category, income) in categorizer.categories() {
        conn.add_category(Category {
            base_category: category
                .split('.')
                .next()
                .expect("split to return at least 1 element")
                .to_string(),
            category: category.to_owned(),
            income: *income,
        })
        .await?;
    }

    // Fill date range
    for d in first_date.iter_days() {
        if d > last_date {
            break;
        }
        conn.add_date(d);
    }

    // Done
    {
        *state.step.lock().expect("lock mutex") = LoadStep::Done;
    }

    Ok(())
}
