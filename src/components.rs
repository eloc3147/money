use rocket::{
    request::Request,
    response::{self, Responder},
    serde::json::Json,
};
use serde::{Deserialize, Serialize};

use crate::error::MoneyError;

#[derive(Debug, Serialize)]
pub struct MoneyMsg<T> {
    status: &'static str,
    response: T,
}

impl<T> MoneyMsg<T> {
    pub fn new(inner: T) -> MoneyMsg<T> {
        MoneyMsg {
            status: "ok",
            response: inner,
        }
    }
}

impl<'r, T: Serialize> Responder<'r, 'static> for MoneyMsg<T> {
    #[inline]
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        Json(self).respond_to(req)
    }
}

pub type MoneyResult<T> = std::result::Result<MoneyMsg<T>, MoneyError>;

#[derive(Clone, PartialEq, Copy, Debug, Serialize, Deserialize)]
pub enum HeaderOption {
    #[serde(rename = "-")]
    Unused,
    Date,
    Name,
    Description,
    Amount,
}

impl HeaderOption {
    pub fn get_header_suggestion(string: &str) -> HeaderOption {
        match string.trim().to_lowercase().as_ref() {
            "date" => HeaderOption::Date,
            "name" => HeaderOption::Name,
            "memo" | "description" => HeaderOption::Description,
            "amount" => HeaderOption::Amount,
            _ => HeaderOption::Unused,
        }
    }
}
