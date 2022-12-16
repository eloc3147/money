use rocket::{
    serde::{json::Json, Deserialize, Serialize},
    Route, State,
};

use crate::components::{MoneyMsg, MoneyResult};
use crate::data_store::SharedDataStore;

#[derive(Debug, Serialize)]
pub struct ListAccountsResponse {
    accounts: Vec<String>,
}

#[get("/")]
pub async fn list_accounts(ds: &State<SharedDataStore>) -> MoneyResult<ListAccountsResponse> {
    let guard = ds.lock().await;
    let accounts = guard.list_accounts();

    Ok(MoneyMsg::new(ListAccountsResponse { accounts }))
}

#[derive(Deserialize)]
struct AddAccountRequest {
    name: String,
}

#[post("/", data = "<account>")]
async fn add_account(
    ds: &State<SharedDataStore>,
    account: Json<AddAccountRequest>,
) -> MoneyResult<()> {
    let account_name = account.name.trim();

    let mut guard = ds.lock().await;
    guard.add_account(account_name)?;

    Ok(MoneyMsg::new(()))
}

pub fn routes() -> Vec<Route> {
    routes![list_accounts, add_account]
}
