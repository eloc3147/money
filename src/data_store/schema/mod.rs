mod v1;

use std::fs::File;
use std::io::{BufReader, Read};
use std::panic;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::error::{MoneyError, Result};

pub use v1::{Account, Data, PendingUpload};

async fn spawn_task<F, R>(f: F) -> Result<R>
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

async fn deserialize_file<T>(path: PathBuf) -> Result<T>
where
    T: DeserializeOwned + Send + 'static,
{
    spawn_task(move || -> Result<T> {
        let reader = BufReader::new(File::open(&path)?);

        bincode::deserialize_from(reader)
            .map_err(|_| MoneyError::DataCorrupted("Data file corrupted"))
    })
    .await?
}

pub async fn load_data(data_dir: &Path) -> Result<Data> {
    let version_file = data_dir.join("version.dat");
    if !version_file.exists() {
        v1::init_data(&data_dir).await?;
    }

    let version = spawn_task(move || -> Result<u16> {
        let mut file = File::open(&version_file)?;
        let mut buf = [0u8; 2];
        file.read_exact(&mut buf)?;

        Ok(u16::from_le_bytes(buf.try_into().unwrap()))
    })
    .await??;

    match version {
        1 => v1::load_data(data_dir).await,
        _ => Err(MoneyError::DataCorrupted("Invalid data version")),
    }
}
