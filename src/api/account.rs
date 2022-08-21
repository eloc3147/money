use rocket::{serde::Serialize, Route};

use crate::models::Account;
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

pub fn routes() -> Vec<Route> {
    routes![list_accounts]
}
