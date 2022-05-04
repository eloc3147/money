use std::fmt;

use csv_async;
use diesel;
use rocket::request::Request;
use yansi::Paint;

macro_rules! error_impl {
    ($enum_name:ident, $(($err_name:ident, $err_msg:expr, $from_type:path)),+) => (
        #[derive(Debug)]
        pub enum $enum_name {
            $($err_name($from_type)),+
        }

        impl ::std::fmt::Display for $enum_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match &self {
                    $($enum_name::$err_name(e) => write!(f, concat!($err_msg, ": {}"), e)),+
                }
            }
        }

        impl<'r> ::rocket::response::Responder<'r, 'static> for $enum_name {
            fn respond_to(self, _: &'r Request<'_>) -> ::rocket::response::Result<'static> {
                match &self {
                    $($enum_name::$err_name(e) => warn_!(concat!($err_msg, ": {}"), Paint::default(e))),+
                }
                ::rocket::response::Result::Err(::rocket::http::Status::InternalServerError)
            }
        }

        $(
            impl From<$from_type> for $enum_name {
                fn from(error: $from_type) -> $enum_name {
                    $enum_name::$err_name(error)
                }
            }
        )+
    );
}

error_impl!(
    MoneyError,
    (IoError, "I/O Error", std::io::Error),
    (DbError, "Database Error", diesel::result::Error),
    (CsvError, "CSV Parsing Error", csv_async::Error),
    (TableError, "Table Access Error", TableError)
);

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
