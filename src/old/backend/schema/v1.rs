use crate::old::error::{MoneyError, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use super::{deserialize_file, serialize_file};

pub struct Data {
    pub accounts: HashMap<String, Account>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Account {
    pub account_name: String,
}

impl Account {
    pub fn new(account_name: String) -> Account {
        Account { account_name }
    }

    pub async fn load(path: PathBuf) -> Result<Account> {
        let account_name = match path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| STANDARD.decode(s).ok())
            .and_then(|b| String::from_utf8(b).ok())
        {
            Some(n) => n,
            None => {
                return Err(MoneyError::DataCorrupted("Invalid account filename"));
            }
        };

        let account: Account = deserialize_file(path).await?;
        if account.account_name != account_name {
            return Err(MoneyError::DataCorrupted("Account name mismatch"));
        }

        Ok(account)
    }

    pub async fn save(self, data_dir: &Path) -> Result<()> {
        let filename = format!("{}.dat", STANDARD.encode(self.account_name.as_bytes()));
        serialize_file(data_dir.join("accounts").join(filename), self).await
    }
}

async fn load_accounts(accounts_dir: &Path) -> Result<HashMap<String, Account>> {
    let mut accounts = HashMap::new();

    let mut read_dir = fs::read_dir(accounts_dir).await?;
    while let Some(item) = read_dir.next_entry().await? {
        let path = item.path();
        if !path.is_file() {
            return Err(MoneyError::DataCorrupted(
                "Unexpected item in accounts directory",
            ));
        }
        match path.extension() {
            Some(e) if e == "dat" => {}
            _ => {
                return Err(MoneyError::DataCorrupted(
                    "Unexpected file extension in accounts directory",
                ));
            }
        }

        let account = Account::load(path).await?;

        match accounts.insert(account.account_name.clone(), account) {
            Some(_) => return Err(MoneyError::DataCorrupted("Account with duplicate name")),
            None => {}
        }
    }

    Ok(accounts)
}

pub async fn load_data(data_dir: &Path) -> Result<Data> {
    let accounts_dir = data_dir.join("accounts");
    let accounts = load_accounts(&accounts_dir).await?;

    Ok(Data { accounts })
}

pub async fn init_data(data_dir: &Path) -> Result<()> {
    if !data_dir.exists() {
        fs::create_dir(&data_dir).await?;
    }

    let mut version_buf = File::create(data_dir.join("version.dat")).await?;
    version_buf.write_u16_le(1).await?;

    fs::create_dir(data_dir.join("accounts")).await?;

    Ok(())
}
