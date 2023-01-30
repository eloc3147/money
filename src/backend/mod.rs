mod schema;
mod upload;

use async_mutex::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

use crate::{
    components::HeaderOption,
    error::{MoneyError, Result},
};
use schema::{load_data, Account, Data};
use upload::PendingUpload;

pub use self::upload::SubmitResult;

pub type BackendHandle = Mutex<Backend>;

pub struct Backend {
    data: Data,
    pending_uploads: HashMap<Uuid, PendingUpload>,
    data_dir: PathBuf,
}

impl Backend {
    pub async fn load(data_dir: PathBuf) -> Result<BackendHandle> {
        let inner = load_data(&data_dir).await?;
        let pending_uploads = HashMap::new();
        Ok(BackendHandle::new(Backend {
            data: inner,
            data_dir,
            pending_uploads,
        }))
    }

    pub fn list_accounts(&self) -> Vec<String> {
        self.data.accounts.keys().map(String::to_owned).collect()
    }

    pub async fn add_account(&mut self, account_name: &str) -> Result<()> {
        if self.data.accounts.contains_key(account_name) {
            return Err(crate::error::MoneyError::AccountAlreadyExists);
        }
        let account = Account::new(account_name.to_string());
        if let Some(_) = self
            .data
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
            if !self.pending_uploads.contains_key(&id) {
                break id;
            }
        };

        let pending_upload = PendingUpload::new(headers, cells, row_count);
        if let Some(_) = self
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
        let upload = match self.pending_uploads.get(&upload_id) {
            Some(u) => u,
            None => return Err(MoneyError::NotFound),
        };

        let cells = upload.get_rows(row_index, row_count)?.to_vec();
        Ok(cells)
    }

    pub fn try_submit_upload(
        &self,
        upload_id: Uuid,
        header_selections: &[HeaderOption],
    ) -> Result<SubmitResult> {
        let upload = match self.pending_uploads.get(&upload_id) {
            Some(u) => u,
            None => return Err(MoneyError::NotFound),
        };

        upload.try_submit(&header_selections)
    }
}
