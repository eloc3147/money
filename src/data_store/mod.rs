mod schema;

use async_mutex::Mutex;
use std::path::PathBuf;
use uuid::Uuid;

use crate::error::{MoneyError, Result};
use schema::{load_data, Account, Data, PendingUpload};

pub type SharedDataStore = Mutex<DataStore>;

pub struct DataStore {
    inner: Data,
    data_dir: PathBuf,
}

impl DataStore {
    pub async fn load(data_dir: PathBuf) -> Result<SharedDataStore> {
        let inner = load_data(&data_dir).await?;
        Ok(SharedDataStore::new(DataStore { inner, data_dir }))
    }

    pub fn list_accounts(&self) -> Vec<String> {
        self.inner.accounts.keys().map(String::to_owned).collect()
    }

    pub async fn add_account(&mut self, account_name: &str) -> Result<()> {
        if self.inner.accounts.contains_key(account_name) {
            return Err(crate::error::MoneyError::AccountAlreadyExists);
        }
        let account = Account::new(account_name.to_string());
        if let Some(_) = self
            .inner
            .accounts
            .insert(account_name.to_string(), account.clone())
        {
            panic!("The account list was modified while locked")
        }

        account.save(&self.data_dir).await?;

        Ok(())
    }

    pub fn add_pending_upload(
        &mut self,
        headers: Vec<String>,
        cells: Vec<String>,
        row_count: usize,
    ) -> Uuid {
        let upload_id = loop {
            let id = Uuid::new_v4();
            if !self.inner.pending_uploads.contains_key(&id) {
                break id;
            }
        };

        let pending_upload = PendingUpload {
            headers,
            cells,
            row_count,
        };
        if let Some(_) = self
            .inner
            .pending_uploads
            .insert(upload_id.clone(), pending_upload)
        {
            unreachable!()
        };

        upload_id
    }

    pub fn get_pending_upload_rows(
        &self,
        upload_id: Uuid,
        row_index: usize,
        row_count: usize,
    ) -> Result<Vec<String>> {
        let upload = match self.inner.pending_uploads.get(&upload_id) {
            Some(u) => u,
            None => return Err(MoneyError::NotFound),
        };

        if row_index > upload.row_count {
            return Err(MoneyError::RowIndex(row_index));
        } else if (row_index + row_count) > upload.row_count {
            return Err(MoneyError::RowIndex(row_index + row_count));
        }

        let start = upload.headers.len() * row_index;
        let end = upload.headers.len() * (row_index + row_count);
        let cells = upload.cells[start..end].to_vec();
        Ok(cells)
    }
}
