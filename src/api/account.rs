use diesel::RunQueryDsl;
use rocket::{
    serde::{json::Json, Deserialize, Serialize},
    Route,
};

use crate::models::{Account, AccountInsert};
use crate::Db;
use crate::{
    components::{MoneyMsg, MoneyResult},
    error::Result,
};

#[derive(Debug, Serialize)]
pub struct ListAccountsResponse {
    accounts: Vec<String>,
}

#[get("/")]
pub async fn list_accounts(db: Db) -> MoneyResult<ListAccountsResponse> {
    let accounts = db
        .run(move |conn| -> Result<Vec<String>> {
            use crate::schema::accounts::dsl::accounts;
            use diesel::prelude::*;

            let account_list = accounts.load::<Account>(conn)?;
            let mut account_names = Vec::with_capacity(account_list.len());
            for account in account_list {
                account_names.push(account.account_name);
            }

            Ok(account_names)
        })
        .await?;

    Ok(MoneyMsg::new(ListAccountsResponse { accounts }))
}

#[derive(Deserialize)]
struct AddAccountRequest {
    name: String,
}

#[post("/", data = "<account>")]
async fn add_account(db: Db, account: Json<AddAccountRequest>) -> MoneyResult<()> {
    let account_name = account.name.trim().to_owned();

    db.run(
        move |conn| -> std::result::Result<(), diesel::result::Error> {
            use crate::schema::accounts::dsl::accounts;

            diesel::insert_into(accounts)
                .values(AccountInsert { account_name })
                .get_result::<Account>(conn)?;

            Ok(())
        },
    )
    .await?;

    Ok(MoneyMsg::new(()))
}

pub fn routes() -> Vec<Route> {
    routes![list_accounts, add_account]
}
