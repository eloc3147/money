mod account;
mod upload;

use rocket::{
    fairing::AdHoc,
    response::{self, Responder},
    serde::json::Json,
    Request,
};
use serde::Serialize;

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

#[catch(404)]
fn not_found(req: &Request) -> MoneyResult<()> {
    Err(MoneyError::MissingEndpoint(
        req.uri().path().as_str().to_string(),
    ))
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Money API", |rocket| async {
        rocket
            .register("/api/", catchers![not_found])
            .mount("/api/upload", upload::routes())
            .mount("/api/account", account::routes())
    })
}
