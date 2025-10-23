use serde::Serialize;

#[derive(Debug, Serialize)]
#[repr(u8)]
pub enum TransactionType {
    Debit,
    Credit,
    Pos,
    Atm,
    Fee,
    Other,
}

#[derive(Debug, Serialize)]
struct Transaction {
    account: String,
    base_category: String,
    category: String,
    source_category: Option<String>,
    income: bool,
    transaction_type: TransactionType,
    date: String,
    amount: f64,
    transaction_id: Option<String>,
    name: String,
    memo: Option<String>,
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
            date: "2011-12-13".into(),
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
            date: "2011-12-14".into(),
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
            date: "2011-12-15".into(),
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
