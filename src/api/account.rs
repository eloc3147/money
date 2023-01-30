use rocket::{
    serde::{json::Json, Deserialize, Serialize},
    Route, State,
};

use crate::api::{MoneyMsg, MoneyResult};
use crate::backend::BackendHandle;

#[derive(Debug, Serialize)]
struct ListAccountsResponse {
    accounts: Vec<String>,
}

#[get("/")]
async fn list_accounts(b: &State<BackendHandle>) -> MoneyResult<ListAccountsResponse> {
    let accounts = {
        let guard = b.lock().await;
        guard.list_accounts()
    };

    Ok(MoneyMsg::new(ListAccountsResponse { accounts }))
}

#[derive(Deserialize)]
struct AddAccountRequest {
    name: String,
}

#[post("/", data = "<account>")]
async fn add_account(
    b: &State<BackendHandle>,
    account: Json<AddAccountRequest>,
) -> MoneyResult<()> {
    let account_name = account.name.trim();

    {
        let mut guard = b.lock().await;
        guard.add_account(account_name).await?;
    }

    Ok(MoneyMsg::new(()))
}

pub fn routes() -> Vec<Route> {
    routes![list_accounts, add_account]
}
