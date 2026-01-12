pub mod categorizer;
mod csv_file;
mod qfx_file;

use std::borrow::Cow;
use std::path::PathBuf;

use categorizer::Categorizer;
use chrono::NaiveDate;
use color_eyre::eyre::{Context, Result, eyre};
use csv_file::CsvReader;
use futures::{StreamExt, TryStreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;

use crate::config::AccountConfig;
use crate::db::{Db, DbHandle};
use crate::importer::categorizer::CategorizationStatus;
use crate::importer::qfx_file::QfxReader;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TransactionType {
    Debit,
    Credit,
    Pos,
    Atm,
    Fee,
    Other,
}

impl TransactionType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Debit => "Debit",
            Self::Credit => "Credit",
            Self::Pos => "Pos",
            Self::Atm => "Atm",
            Self::Fee => "Fee",
            Self::Other => "Other",
        }
    }
}

#[derive(Debug)]
pub struct Transaction<'a> {
    pub transaction_type: TransactionType,
    pub date_posted: NaiveDate,
    pub amount: Decimal,
    pub transaction_id: Option<Cow<'a, str>>,
    pub category: Option<Cow<'a, str>>,
    pub name: Cow<'a, str>,
    pub memo: Option<Cow<'a, str>>,
}

async fn list_accounts(
    accounts: &[AccountConfig],
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
                    file_queue
                        .send((account.name.clone(), entry.path()))
                        .await?;
                } else if entry_type.is_symlink() {
                    let new_path = tokio::fs::read_link(entry.path()).await?;
                    let new_meta = tokio::fs::metadata(&new_path).await?;

                    if new_meta.is_file() {
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

pub trait TransactionReader {
    async fn load(self, importer: TransactionImporter<'_>) -> Result<()>;
}

struct ImportConfig<'a> {
    db: &'a Db,
    categorizer: &'a Categorizer,
    account_name: String,
    file_path: PathBuf,
}

pub struct TransactionImporter<'c> {
    conn: DbHandle,
    categorizer: &'c Categorizer,
    account_name: String,
}

impl<'c> TransactionImporter<'c> {
    pub async fn import<'t>(&mut self, transaction: Transaction<'t>) -> Result<()> {
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
                self.conn.add_uncategorized_transaction(t).await?;
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

async fn import_file(config: ImportConfig<'_>) -> Result<()> {
    let ext = config
        .file_path
        .extension()
        .ok_or_else(|| eyre!("File missing extension: {:?}", config.file_path))?
        .to_ascii_lowercase();

    let importer = TransactionImporter {
        conn: config.db.open_handle().await?,
        categorizer: config.categorizer,
        account_name: config.account_name,
    };

    match &*ext.to_string_lossy() {
        "qfx" => {
            QfxReader::open(&config.file_path)
                .await
                .wrap_err_with(|| {
                    format!(
                        "Failed to open file: {}",
                        config.file_path.to_string_lossy()
                    )
                })?
                .load(importer)
                .await
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
                .load(importer)
                .await
        }
        ext => return Err(eyre!("Unrecognized file type: {}", ext)),
    }
}

pub async fn import_files(
    db: &Db,
    categorizer: &Categorizer,
    accounts: &[AccountConfig],
) -> Result<()> {
    // Load transactions concurrently
    let (file_tx, file_rx) = tokio::sync::mpsc::channel(8);

    let account_listing = list_accounts(accounts, file_tx);
    let file_loading = ReceiverStream::new(file_rx)
        .map(|(account_name, file_path)| {
            // Funky stuff to get all required state to the concurrent function
            Ok(ImportConfig {
                db,
                categorizer,
                account_name,
                file_path,
            })
        })
        .try_for_each_concurrent(8, import_file);

    futures::future::try_join(account_listing, file_loading).await?;
    Ok(())
}
