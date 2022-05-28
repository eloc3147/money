use std::fmt;

use csv_async;
use diesel;
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
    DbError(diesel::result::Error),
    CsvError(csv_async::Error),
    TableError(TableError),
    MissingEndpoint(String),
    InvalidUuid(uuid::Error),
}

impl MoneyError {
    pub fn msg(&self) -> &'static str {
        match self {
            MoneyError::IoError(_) => "I/O Error",
            MoneyError::DbError(_) => "Database Error",
            MoneyError::CsvError(_) => "CSV Parsing Error",
            MoneyError::TableError(_) => "Table Access Error",
            MoneyError::MissingEndpoint(_) => "Endpoint not found",
            MoneyError::InvalidUuid(_) => "Invalid UUID",
        }
    }
}

impl fmt::Display for MoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            MoneyError::IoError(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::DbError(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::CsvError(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::TableError(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::MissingEndpoint(e) => write!(f, "{}: {}", self.msg(), e),
            MoneyError::InvalidUuid(e) => write!(f, "{}: {}", self.msg(), e),
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

impl From<diesel::result::Error> for MoneyError {
    fn from(error: diesel::result::Error) -> MoneyError {
        MoneyError::DbError(error)
    }
}

impl From<csv_async::Error> for MoneyError {
    fn from(error: csv_async::Error) -> MoneyError {
        MoneyError::CsvError(error)
    }
}

impl From<uuid::Error> for MoneyError {
    fn from(error: uuid::Error) -> MoneyError {
        MoneyError::InvalidUuid(error)
    }
}

impl From<TableError> for MoneyError {
    fn from(error: TableError) -> MoneyError {
        MoneyError::TableError(error)
    }
}

#[derive(Debug)]
pub enum TableError {
    RowIndex { row: usize, bound: usize },
    RowLength { length: usize, bound: usize },
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            TableError::RowIndex { row, bound } => {
                write!(f, "Row {} is outside length {}", row, bound)
            }
            TableError::RowLength { length, bound } => {
                write!(
                    f,
                    "Row has length {} which differs from column count {}",
                    length, bound
                )
            }
        }
    }
}

impl std::error::Error for TableError {}

pub type Result<T> = std::result::Result<T, MoneyError>;
