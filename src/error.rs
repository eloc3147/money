use std::fmt;

use csv_async;
use diesel;
use rocket::http::Status;
use rocket::request::Request;
use rocket::response::{self, Responder};
use yansi::Paint;

#[derive(Debug)]
pub enum MoneyError {
    IoError(std::io::Error),
    DbError(diesel::result::Error),
    CsvError(csv_async::Error),
}

impl fmt::Display for MoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            MoneyError::IoError(e) => write!(f, "I/O Error: {}", e),
            MoneyError::DbError(e) => write!(f, "Database Error: {}", e),
            MoneyError::CsvError(e) => write!(f, "CSV Parsing Error: {}", e),
        }
    }
}

impl<'r> Responder<'r, 'static> for MoneyError {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        match &self {
            MoneyError::IoError(e) => warn_!("I/O Error: {:?}", Paint::default(e)),
            MoneyError::DbError(e) => warn_!("Database Error: {:?}", Paint::default(e)),
            MoneyError::CsvError(e) => warn_!("CSV Parsing Error: {:?}", Paint::default(e)),
        }
        Err(Status::InternalServerError)
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

impl std::error::Error for MoneyError {}

pub type Result<T> = std::result::Result<T, MoneyError>;
