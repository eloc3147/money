use rocket::{
    futures::TryStreamExt,
    serde::{json::Json, Deserialize, Serialize},
    Route,
};
use rocket_db_pools::sqlx::{self, Row};

use crate::{
    api::{ApiResponse, ApiResult},
    backend::db::Db,
};

#[derive(Debug, Serialize)]
struct ListAccountsResponse {
    accounts: Vec<String>,
}

#[get("/")]
async fn list_accounts(db: &Db) -> ApiResult<ListAccountsResponse> {
    let mut rows = sqlx::query("SELECT name FROM accounts;").fetch(&**db);

    let mut accounts = Vec::new();
    while let Some(row) = rows.try_next().await? {
        accounts.push(row.try_get::<String, usize>(0)?);
    }

    Ok(ApiResponse::new(ListAccountsResponse { accounts }))
}

#[derive(Deserialize)]
struct AddAccountRequest {
    name: String,
}

#[post("/", data = "<account>")]
async fn add_account(db: &Db, account: Json<AddAccountRequest>) -> ApiResult<()> {
    let account_name = account.name.trim();

    sqlx::query("INSERT INTO accounts (name) VALUES (?);")
        .bind(account_name)
        .execute(&**db)
        .await?;

    Ok(ApiResponse::new(()))
}

pub fn routes() -> Vec<Route> {
    routes![list_accounts, add_account]
}
