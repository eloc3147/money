use std::fmt;

use rocket::{
    request::Request,
    response::{self, Responder},
    serde::{
        de::{self, Visitor},
        json::Json,
        Deserialize, Deserializer, Serialize, Serializer,
    },
};

use crate::error::{MoneyError, Result};

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

#[derive(Clone, PartialEq, Copy, Debug)]
pub enum HeaderOption {
    Unused,
    Date,
    Name,
    Description,
    Amount,
}

impl HeaderOption {
    pub fn to_str(&self) -> &'static str {
        match self {
            HeaderOption::Unused => "-",
            HeaderOption::Date => "Date",
            HeaderOption::Name => "Name",
            HeaderOption::Description => "Description",
            HeaderOption::Amount => "Amount",
        }
    }

    pub fn from_str(string: &str) -> HeaderOption {
        match string.trim().to_lowercase().as_ref() {
            "date" => HeaderOption::Date,
            "name" => HeaderOption::Name,
            "memo" | "description" => HeaderOption::Description,
            "amount" => HeaderOption::Amount,
            _ => HeaderOption::Unused,
        }
    }
}

impl Serialize for HeaderOption {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_str())
    }
}

struct HeaderOptionVisitor;

impl<'de> Visitor<'de> for HeaderOptionVisitor {
    type Value = HeaderOption;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(HeaderOption::from_str(value))
    }
}

impl<'de> Deserialize<'de> for HeaderOption {
    fn deserialize<D>(deserializer: D) -> std::result::Result<HeaderOption, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(HeaderOptionVisitor)
    }
}
