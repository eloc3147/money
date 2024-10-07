mod account;
mod upload;

use anyhow::{anyhow, Error};
use rocket::{
    fairing::AdHoc,
    http::Status,
    response::{self, Responder},
    serde::json::Json,
    Request,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    status: &'static str,
    response: T,
}

impl<T> ApiResponse<T> {
    pub fn new(inner: T) -> ApiResponse<T> {
        ApiResponse {
            status: "ok",
            response: inner,
        }
    }
}

impl<'r, T: Serialize> Responder<'r, 'static> for ApiResponse<T> {
    #[inline]
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        Json(self).respond_to(req)
    }
}

#[derive(Serialize, Debug)]
struct MoneyErrorMsg {
    status: &'static str,
    msg: String,
}

pub struct ApiError(anyhow::Error);

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl From<rocket_db_pools::sqlx::Error> for ApiError {
    fn from(error: rocket_db_pools::sqlx::Error) -> Self {
        Self(error.into())
    }
}

impl<'r> Responder<'r, 'static> for ApiError {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        let msg = format!("{:#?}", self.0);
        warn!("{}", &msg);

        let mut resp = Json(MoneyErrorMsg {
            status: "error",
            msg: msg,
        })
        .respond_to(req)?;
        resp.set_status(Status::InternalServerError);
        Ok(resp)
    }
}

pub type ApiResult<T> = Result<ApiResponse<T>, ApiError>;

#[catch(404)]
fn not_found(req: &Request) -> ApiResult<()> {
    Err(anyhow!("Unknown endpoint: {}", req.uri().path().as_str()).into())
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Money API", |rocket| async {
        rocket
            .register("/api/", catchers![not_found])
            .mount("/api/account", account::routes())
            .mount("/api/upload", upload::routes())
    })
}
