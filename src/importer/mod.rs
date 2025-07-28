pub mod config;
pub mod loader;

pub mod categorizer;
mod qfx;

use crate::data::DataStore;

use categorizer::Categorizer;
use color_eyre::eyre::{Context, Result, eyre};
use config::AccountConfig;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use loader::Loader;

pub fn import_data(
    categorizer: &mut Categorizer,
    accounts: &[AccountConfig],
    progress: &MultiProgress,
) -> Result<DataStore> {
    let mut loader = Loader::new();

    let spinner = progress.add(ProgressBar::no_length());
    spinner.set_style(ProgressStyle::default_spinner());
    spinner.tick();

    let mut data = DataStore::new().wrap_err("Failed to create data store")?;

    for account in accounts {
        spinner.set_message(format!("Loading account {}", account.name));
        spinner.tick();

        let account_id = data
            .add_account(&account.name)
            .wrap_err("Failed to add account to data store")?;
        let mut insert_handle = data
            .build_batch_insert_handle()
            .wrap_err("Failed to create batch insert handle")?;

        let account_spinner = progress.add(ProgressBar::no_length());
        account_spinner.set_style(ProgressStyle::default_spinner());
        account_spinner.tick();

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

            let transactions = reader.transactions()?;
            for transaction in transactions {
                let transaction = transaction?;

                if transaction.amount == 0.0 && transaction.transaction_id.contains('.') {
                    // Weird multiline transaction. Extra lines don't contain much useful information
                    continue;
                }

                let category = categorizer
                    .categorize(&account.name, &transaction.name, transaction.memo)?
                    .unwrap_or("Uncategorized");

                insert_handle
                    .add_transaction(
                        account_id,
                        category,
                        transaction.transaction_type,
                        transaction.date_posted,
                        transaction.amount,
                        transaction.transaction_id,
                        transaction.name,
                        transaction.memo,
                    )
                    .wrap_err("Failed to add transaction")?;
            }
        }
    }

    Ok(data)
}
