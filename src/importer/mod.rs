pub mod loader;

pub mod categorizer;
mod csv_file;
mod qfx_file;

use std::borrow::Cow;

use categorizer::Categorizer;
use chrono::NaiveDate;
use color_eyre::eyre::{Context, Result, eyre};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use loader::Loader;
use serde::{Deserialize, Serialize};

use crate::config::AccountConfig;
use crate::db::DbConnection;

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
    pub amount: f64,
    pub transaction_id: Option<Cow<'a, str>>,
    pub category: Option<Cow<'a, str>>,
    pub name: Cow<'a, str>,
    pub memo: Option<Cow<'a, str>>,
}

pub async fn import_data(
    categorizer: &mut Categorizer,
    conn: &mut DbConnection,
    accounts: &[AccountConfig],
    progress: &MultiProgress,
) -> Result<()> {
    let mut loader = Loader::new();

    let spinner = progress.add(ProgressBar::no_length());
    spinner.set_style(ProgressStyle::default_spinner());
    spinner.tick();

    // Add transactions
    for account in accounts {
        spinner.set_message(format!("Loading account {}", account.name));
        spinner.tick();

        let account_spinner = progress.add(ProgressBar::no_length());
        account_spinner.set_style(ProgressStyle::default_spinner());
        account_spinner.tick();

        loader.clear();
        loader.add_dir(&account.source_path).wrap_err_with(|| {
            format!(
                "Error searching account dir: {}",
                account.source_path.to_string_lossy()
            )
        })?;

        let base_path = account.source_path.parent().ok_or_else(|| {
            eyre!(
                "Invalid source path: {}",
                &account.source_path.to_string_lossy()
            )
        })?;
        loop {
            let Some((file_path, mut reader)) = loader
                .open_next_file()
                .wrap_err("Error opening account file")?
            else {
                break;
            };

            account_spinner.set_message(format!(
                "Loading {}",
                file_path.strip_prefix(base_path).unwrap().to_string_lossy()
            ));
            account_spinner.tick();

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
                    &account.name,
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

                conn.add_transaction(&account.name, categorization, transaction)
                    .await?;
            }
        }

        account_spinner.finish();
        progress.remove(&account_spinner);
    }

    spinner.set_message("Loading metadata");

    // Add categories
    for (category, income) in categorizer.categories() {
        conn.add_category(category, *income).await?;
    }

    spinner.finish();
    progress.remove(&spinner);

    Ok(())
}
