use async_mutex::Mutex;
use std::{collections::HashMap, path::Path};
use uuid::Uuid;

use crate::error::MoneyError;

pub type SharedDataStore = Mutex<DataStore>;

pub struct DataStore {
    pending_uploads: HashMap<Uuid, PendingUpload>,
    accounts: HashMap<String, Account>,
}

impl DataStore {
    pub fn load(data_dir: &Path) -> SharedDataStore {
        let data = DataStore {
            accounts: HashMap::new(),
            pending_uploads: HashMap::new(),
        };
        SharedDataStore::new(data)
    }

    pub fn list_accounts(&self) -> Vec<String> {
        self.accounts.keys().map(String::to_owned).collect()
    }

    pub fn add_account(&mut self, account_name: &str) -> Result<(), MoneyError> {
        if self.accounts.contains_key(account_name) {
            return Err(crate::error::MoneyError::AccountAlreadyExists);
        }
        let account = Account::new(account_name.to_string());
        if let Some(_) = self.accounts.insert(account_name.to_string(), account) {
            panic!("The account list was modified while locked")
        }
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

        let pending_upload = PendingUpload {
            headers,
            cells,
            row_count,
        };
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
    ) -> Result<Vec<String>, MoneyError> {
        let upload = match self.pending_uploads.get(&upload_id) {
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

pub struct Account {
    account_name: String,
    transactions: Vec<bool>,
}

impl Account {
    pub fn new(account_name: String) -> Account {
        Account {
            account_name,
            transactions: Vec::new(),
        }
    }
}

struct PendingUpload {
    headers: Vec<String>,
    cells: Vec<String>,
    row_count: usize,
}
