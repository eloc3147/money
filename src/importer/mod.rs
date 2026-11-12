//! Importing of bank statement files (QFX and CSV) into the database.

pub mod categorizer;
mod csv_file;
mod qfx_file;

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use categorizer::Categorizer;
use chrono::NaiveDate;
use color_eyre::eyre::{Context, Result, eyre};
use csv_file::CsvReader;
use futures::{StreamExt, TryStreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr, VariantArray};
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;

use crate::config::AccountConfig;
use crate::db::{Db, DbTransaction};
use crate::importer::categorizer::CategorizationStatus;
use crate::importer::qfx_file::QfxReader;

/// The raw transaction type reported by a source file.
#[derive(
    Debug,
    Deserialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    PartialOrd,
    Ord,
    VariantArray,
    IntoStaticStr,
    EnumString,
)]
#[allow(missing_docs)]
pub enum TransactionType {
    Debit,
    Credit,
    Pos,
    Atm,
    Fee,
    Other,
}

impl TransactionType {
    /// The name used for this type in the config file and database.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.into()
    }

    /// Parse a name produced by [`Self::name`].
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        name.parse().ok()
    }
}

/// A transaction as read from a source file, before categorization.
#[derive(Debug)]
pub struct Transaction<'a> {
    /// Type reported by the file.
    pub transaction_type: TransactionType,
    /// Date posted.
    pub date_posted: NaiveDate,
    /// Signed amount.
    pub amount: Decimal,
    /// Bank transaction id, if provided.
    pub transaction_id: Option<Cow<'a, str>>,
    /// Bank category, if provided.
    pub category: Option<Cow<'a, str>>,
    /// Transaction name.
    pub name: Cow<'a, str>,
    /// Transaction memo, if provided.
    pub memo: Option<Cow<'a, str>>,
}

/// Import progress counters, safe to read from another thread while an import runs.
#[derive(Debug, Default)]
pub struct ImportProgress {
    /// Number of files discovered so far.
    files_found: AtomicUsize,
    /// Number of files finished (imported or skipped).
    files_done: AtomicUsize,
    /// Number of transactions read so far.
    transactions: AtomicUsize,
}

impl ImportProgress {
    /// Number of files discovered so far.
    #[must_use]
    pub fn files_found(&self) -> usize {
        self.files_found.load(Ordering::Relaxed)
    }

    /// Number of files finished.
    #[must_use]
    pub fn files_done(&self) -> usize {
        self.files_done.load(Ordering::Relaxed)
    }

    /// Number of transactions read.
    #[must_use]
    pub fn transactions(&self) -> usize {
        self.transactions.load(Ordering::Relaxed)
    }

    /// Count one more transaction read.
    pub fn add_transaction(&self) {
        self.transactions.fetch_add(1, Ordering::Relaxed);
    }
}

/// Recursively list the files of every account, sending `(account, path)` to `file_queue`.
async fn list_accounts(
    accounts: &[AccountConfig],
    file_queue: Sender<(String, PathBuf)>,
    progress: &ImportProgress,
) -> Result<()> {
    let mut stack = Vec::new();
    for account in accounts {
        stack.push(account.source_path.clone());

        while let Some(dir) = stack.pop() {
            let mut read_dir = tokio::fs::read_dir(&dir).await.wrap_err_with(|| {
                format!(
                    "Failed to read directory {} for account {}",
                    dir.display(),
                    account.name
                )
            })?;

            while let Some(entry) = read_dir.next_entry().await? {
                let entry_type = entry.file_type().await?;

                if entry_type.is_dir() {
                    stack.push(entry.path());
                } else if entry_type.is_file() {
                    progress.files_found.fetch_add(1, Ordering::Relaxed);

                    file_queue
                        .send((account.name.clone(), entry.path()))
                        .await?;
                } else if entry_type.is_symlink() {
                    let new_path = tokio::fs::read_link(entry.path()).await?;
                    let new_meta = tokio::fs::metadata(&new_path).await?;

                    if new_meta.is_file() {
                        progress.files_found.fetch_add(1, Ordering::Relaxed);

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

/// A source file format which can feed transactions to a [`TransactionImporter`].
pub(crate) trait TransactionReader {
    /// Read every transaction in the file and pass it to `importer`.
    async fn load(self, importer: TransactionImporter, progress: &ImportProgress) -> Result<()>;
}

/// Everything needed to import a single file.
struct FileImportConfig<'a> {
    /// Destination database.
    db: &'a Db,
    /// Categorizer for the file's transactions.
    categorizer: &'a Categorizer,
    /// Account the file belongs to.
    account_name: String,
    /// The file to import.
    file_path: PathBuf,
    /// Shared progress counters.
    progress: &'a ImportProgress,
}

/// Categorizes transactions and stores them within one database transaction.
pub struct TransactionImporter<'a, 'd> {
    /// The open database transaction.
    conn: &'a mut DbTransaction<'d>,
    /// Categorizer used for every transaction.
    categorizer: &'a Categorizer,
    /// Account the transactions belong to.
    account_name: String,
}

impl TransactionImporter<'_, '_> {
    /// Categorize and store one transaction.
    ///
    /// # Errors
    /// Fails if categorization fails or the database write fails.
    pub async fn import(&mut self, transaction: Transaction<'_>) -> Result<()> {
        if let Some(tid) = transaction.transaction_id.as_ref()
            && tid.contains(".")
            && transaction.amount.is_zero()
        {
            // Weird multiline transaction. Extra lines don't contain much useful information
            return Ok(());
        }

        let categorization_result = self.categorizer.categorize(
            &self.account_name,
            &transaction.name,
            transaction.transaction_type,
            transaction.memo.as_ref().map(|m| m.as_ref()),
        )?;
        let categorization = match categorization_result {
            CategorizationStatus::Categorized(c) => c,
            CategorizationStatus::Uncategorized(t) => {
                self.conn
                    .add_uncategorized_transaction(t, transaction.date_posted, transaction.amount)
                    .await?;
                return Ok(());
            }
        };

        if categorization.ignore {
            return Ok(());
        }

        self.conn
            .add_transaction(&self.account_name, categorization, transaction)
            .await?;

        Ok(())
    }
}

/// Import one file in a single database transaction, skipping already imported file names.
async fn import_file(config: FileImportConfig<'_>) -> Result<()> {
    let mut db_handle = config.db.start_transaction().await?;

    let file_name = config
        .file_path
        .file_name()
        .ok_or_else(|| eyre!("Missing file name: {:?}", config.file_path))?
        .to_str()
        .ok_or_else(|| eyre!("Filename is not valid utf-8: {:?}", config.file_path))?;

    if db_handle.check_loaded_file(file_name).await? {
        config.progress.files_done.fetch_add(1, Ordering::Relaxed);
        return Ok(());
    }

    let ext = config
        .file_path
        .extension()
        .ok_or_else(|| eyre!("File missing extension: {:?}", config.file_path))?
        .to_ascii_lowercase();

    let importer = TransactionImporter {
        conn: &mut db_handle,
        categorizer: config.categorizer,
        account_name: config.account_name,
    };

    match &*ext.to_string_lossy() {
        "qfx" | "ofx" => {
            QfxReader::open(&config.file_path)
                .await
                .wrap_err_with(|| {
                    format!(
                        "Failed to open file: {}",
                        config.file_path.to_string_lossy()
                    )
                })?
                .load(importer, config.progress)
                .await?;
        }
        "csv" => {
            CsvReader::open(&config.file_path)
                .await
                .wrap_err_with(|| {
                    format!(
                        "Failed to open file: {}",
                        config.file_path.to_string_lossy()
                    )
                })?
                .load(importer, config.progress)
                .await?;
        }
        ext => return Err(eyre!("Unrecognized file type: {}", ext)),
    }

    db_handle.add_loaded_file(file_name).await?;
    db_handle.commit().await?;

    config.progress.files_done.fetch_add(1, Ordering::Relaxed);

    Ok(())
}

/// Import every file of every account into `db`, up to 8 files at a time.
///
/// # Errors
/// Fails if any directory cannot be listed or any file fails to import.
pub async fn import_files(
    db: &Db,
    categorizer: &Categorizer,
    accounts: &[AccountConfig],
    progress: &ImportProgress,
) -> Result<()> {
    // Load transactions concurrently
    let (file_tx, file_rx) = tokio::sync::mpsc::channel(8);

    let account_listing = list_accounts(accounts, file_tx, progress);
    let file_loading = ReceiverStream::new(file_rx)
        .map(|(account_name, file_path)| {
            // Funky stuff to get all required state to the concurrent function
            Ok(FileImportConfig {
                db,
                categorizer,
                account_name,
                file_path,
                progress,
            })
        })
        .try_for_each_concurrent(8, import_file);

    futures::future::try_join(account_listing, file_loading).await?;
    Ok(())
}
