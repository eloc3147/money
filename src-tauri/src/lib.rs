mod db;
mod error;
mod loader;

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::NaiveDate;

use crate::db::{Transaction, TransactionType};

#[derive(Debug, Clone, Copy)]
pub enum LoadStep {
    NotStarted,
    LoadingConfig,
    BuildingRules,
    LoadingFiles,
    Done,
}

pub struct LoadState {
    pub step: Mutex<LoadStep>,
    pub loaded_count: AtomicUsize,
    pub total_count: AtomicUsize,
}

impl LoadState {
    fn progress(&self) -> f32 {
        let step = { *self.step.lock().expect("mutex to lock") };

        match step {
            LoadStep::NotStarted => 0.0,
            LoadStep::LoadingConfig => 1.0,
            LoadStep::BuildingRules => 5.0,
            LoadStep::LoadingFiles => {
                let loaded = self.loaded_count.load(Ordering::Relaxed);
                let total = self.total_count.load(Ordering::Relaxed);
                if total == 0 {
                    return 10.0;
                }
                10.0 + (loaded as f32 / total as f32) * 85.0
            }
            LoadStep::Done => 100.0,
        }
    }
}

struct AppState {
    load_state: LoadState,
}

#[tauri::command]
fn fetch_transactions() -> Vec<Transaction> {
    vec![
        Transaction {
            account: "Checking".into(),
            base_category: "Food".into(),
            category: "Food.Restaurant".into(),
            source_category: None,
            income: false,
            transaction_type: TransactionType::Debit,
            date: NaiveDate::from_ymd_opt(2011, 12, 13).unwrap(),
            amount: 77.1,
            transaction_id: Some("12345".into()),
            name: "Joe's BBQ".into(),
            memo: Some("Transaction at 2pm".into()),
        },
        Transaction {
            account: "Checking".into(),
            base_category: "Food".into(),
            category: "Food.Restaurant".into(),
            source_category: None,
            income: false,
            transaction_type: TransactionType::Debit,
            date: NaiveDate::from_ymd_opt(2011, 12, 14).unwrap(),
            amount: 11.23,
            transaction_id: None,
            name: "Joe's Halal Truck".into(),
            memo: Some("Transaction at 2am".into()),
        },
        Transaction {
            account: "Checking".into(),
            base_category: "Food".into(),
            category: "Food.Restaurant".into(),
            source_category: None,
            income: false,
            transaction_type: TransactionType::Debit,
            date: NaiveDate::from_ymd_opt(2011, 12, 15).unwrap(),
            amount: 294.75,
            transaction_id: Some("55555".into()),
            name: "Joe's Samosa".into(),
            memo: None,
        },
    ]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![fetch_transactions])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
