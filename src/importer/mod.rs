pub mod config;
pub mod loader;

pub mod categorizer;
mod qfx;

use categorizer::Categorizer;
use chrono::{Days, NaiveDate};
use color_eyre::eyre::{Context, Result, bail, eyre};
use config::AccountConfig;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use loader::Loader;
use num_enum::IntoPrimitive;
use serde::Deserialize;

use crate::db::DbConnection;

#[derive(Debug, IntoPrimitive, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TransactionType {
    Debit,
    Credit,
    Pos,
    Atm,
    Fee,
    Other,
}

#[derive(Debug)]
pub struct Transaction<'a> {
    pub transaction_type: TransactionType,
    pub date_posted: NaiveDate,
    pub amount: f64,
    pub transaction_id: &'a str,
    pub name: &'a str,
    pub memo: Option<&'a str>,
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
    let mut first_date = NaiveDate::MAX;
    let mut last_date = NaiveDate::MIN;
    for account in accounts {
        spinner.set_message(format!("Loading account {}", account.name));
        spinner.tick();

        // Get account ID
        let account_id = conn.add_account(account.name.clone()).await?;

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
            let Some((file_path, reader)) = loader
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

                if transaction.amount == 0.0 && transaction.transaction_id.contains('.') {
                    // Weird multiline transaction. Extra lines don't contain much useful information
                    continue;
                }

                let categorization_result = categorizer.categorize(
                    &account.name,
                    &transaction.name,
                    transaction.transaction_type,
                    transaction.memo,
                )?;
                let Some(categorization) = categorization_result else {
                    if transaction.name == "PAYMENT" {
                        dbg!(file_path, account, transaction);
                        bail!("E");
                    }
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

                conn.add_transaction(
                    account_id,
                    categorization.category,
                    categorization.income,
                    transaction.transaction_type,
                    transaction.date_posted,
                    transaction.amount,
                    transaction.transaction_id,
                    transaction.name,
                    transaction.memo,
                )
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

    spinner.tick();

    // Fill date range
    let mut add_date = first_date;
    while add_date <= last_date {
        conn.add_date(add_date).await?;
        add_date = add_date + Days::new(1);
    }

    spinner.finish();
    progress.remove(&spinner);

    Ok(())
}
