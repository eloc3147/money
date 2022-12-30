use rocket::{
    request::Request,
    response::{self, Responder},
    serde::{json::Json, Deserialize, Serialize},
};
use serde::{de::Visitor, Deserializer};

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

#[derive(Clone, PartialEq, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HeaderOption {
    #[serde(rename = "-")]
    Unused,
    Date,
    Name,
    Description,
    Amount,
}

impl HeaderOption {
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

impl<'de> Deserialize<'de> for HeaderOption {
    fn deserialize<D>(deserializer: D) -> Result<HeaderOption, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(HeaderOptionVisitor)
    }
}

struct HeaderOptionVisitor;

impl<'de> Visitor<'de> for HeaderOptionVisitor {
    type Value = HeaderOption;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a string header name")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(HeaderOption::from_str(value))
    }
}
