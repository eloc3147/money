use std::panic;
use std::path::Path;

use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, BufReader};

use crate::error::{MoneyError, Result};

pub async fn spawn_task<F, R>(f: F) -> Result<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => Ok(r),
        Err(e) => {
            if e.is_panic() {
                panic::resume_unwind(e.into_panic())
            }
            Err(MoneyError::OperationCancelled)
        }
    }
}

mod schema_v1 {
    use crate::error::{MoneyError, Result};
    use base64;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::path::Path;
    use tokio::fs::{self, File};
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
    use uuid::Uuid;

    use super::spawn_task;

    pub struct Data {
        pub pending_uploads: HashMap<Uuid, PendingUpload>,
        pub accounts: HashMap<String, Account>,
    }

    #[derive(Deserialize, Serialize)]
    pub struct Account {
        pub account_name: String,
    }

    impl Account {
        pub fn new(account_name: String) -> Account {
            Account { account_name }
        }
    }

    pub struct PendingUpload {
        pub headers: Vec<String>,
        pub cells: Vec<String>,
        pub row_count: usize,
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
            let account_name = match path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|s| base64::decode(s).ok())
                .and_then(|b| String::from_utf8(b).ok())
            {
                Some(n) => n,
                None => {
                    return Err(MoneyError::DataCorrupted(
                        "Unexpected file name in accounts directory",
                    ));
                }
            };

            let stats = fs::metadata(&path).await?;
            let mut reader = BufReader::new(File::open(&path).await?);
            let mut buf = Vec::with_capacity(stats.len() as usize);
            reader.read_to_end(&mut buf).await?;

            let account = spawn_task(move || -> Result<Account> {
                bincode::deserialize(&buf)
                    .map_err(|_| MoneyError::DataCorrupted("Account data corrupted"))
            })
            .await??;

            if account.account_name != account_name {
                return Err(MoneyError::DataCorrupted("Account name mismatch"));
            }
            match accounts.insert(account_name, account) {
                Some(_) => return Err(MoneyError::DataCorrupted("Account with duplicate name")),
                None => {}
            }
        }

        Ok(accounts)
    }

    pub async fn load_data(data_dir: &Path) -> Result<Data> {
        let accounts_dir = data_dir.join("accounts");
        if !accounts_dir.exists() {
            fs::create_dir(&accounts_dir).await?;
        }

        let accounts = load_accounts(&accounts_dir).await?;
        let pending_uploads = HashMap::new();

        Ok(Data {
            pending_uploads,
            accounts,
        })
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
}

pub async fn load_data(data_dir: &Path) -> Result<Data> {
    let version_file = data_dir.join("version.dat");
    if !version_file.exists() {
        schema_v1::init_data(&data_dir).await?;
    }

    let mut reader = BufReader::new(File::open(&version_file).await?);
    let version = reader.read_u16_le().await?;
    match version {
        1 => schema_v1::load_data(data_dir).await,
        _ => Err(MoneyError::DataCorrupted("Invalid data version")),
    }
}

pub use schema_v1::{Account, Data, PendingUpload};
