use std::fmt;

use csv_async;
use rocket::{
    http::Status,
    request::Request,
    response::{self, Responder},
    serde::{json::Json, Serialize},
};
use uuid;
use yansi::Paint;

#[derive(Serialize, Debug)]
struct MoneyErrorMsg {
    status: &'static str,
    msg: &'static str,
}

#[derive(Debug)]
pub enum MoneyError {
    IoError(std::io::Error),
    CsvError(csv_async::Error),
    MissingEndpoint(String),
    InvalidUuid(uuid::Error),
    RowIndex(usize),
    AccountAlreadyExists,
    NotFound,
}

impl MoneyError {
    pub fn msg(&self) -> &'static str {
        match self {
            MoneyError::IoError(_) => "I/O Error",
            MoneyError::CsvError(_) => "CSV Parsing Error",
            MoneyError::MissingEndpoint(_) => "Endpoint not found",
            MoneyError::InvalidUuid(_) => "Invalid UUID",
            MoneyError::RowIndex(_) => "Requested row does not exist",
            MoneyError::AccountAlreadyExists => "Account with that name already exists",
            MoneyError::NotFound => "The requested item was not found",
        }
    }

    pub fn context(&self) -> Option<String> {
        match self {
            MoneyError::CsvError(e) => Some(e.to_string()),
            MoneyError::MissingEndpoint(endpoint) => Some(endpoint.clone()),
            MoneyError::RowIndex(row) => Some(row.to_string()),
            _ => None,
        }
    }
}

impl fmt::Display for MoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            MoneyError::IoError(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::CsvError(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::MissingEndpoint(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::InvalidUuid(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::RowIndex(r) => write!(f, "{}: {}", self.msg(), r),
            MoneyError::AccountAlreadyExists | MoneyError::NotFound => write!(f, "{}", self.msg()),
        }
    }
}

impl<'r> Responder<'r, 'static> for MoneyError {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        warn_!("{}", Paint::default(&self));

        let mut resp = Json(MoneyErrorMsg {
            status: "error",
            msg: self.msg(),
        })
        .respond_to(req)?;
        resp.set_status(Status::InternalServerError);
        Ok(resp)
    }
}

impl From<std::io::Error> for MoneyError {
    fn from(error: std::io::Error) -> MoneyError {
        MoneyError::IoError(error)
    }
}

impl From<csv_async::Error> for MoneyError {
    fn from(error: csv_async::Error) -> MoneyError {
        if error.is_io_error() {
            match error.into_kind() {
                csv_async::ErrorKind::Io(e) => MoneyError::IoError(e),
                _ => unreachable!(),
            }
        } else {
            MoneyError::CsvError(error)
        }
    }
}

impl From<uuid::Error> for MoneyError {
    fn from(error: uuid::Error) -> MoneyError {
        MoneyError::InvalidUuid(error)
    }
}

pub type Result<T> = std::result::Result<T, MoneyError>;
