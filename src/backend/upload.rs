use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use serde_variant::to_variant_name;

use crate::error::MoneyError;

#[derive(Clone, PartialEq, Copy, Debug, Serialize, Deserialize, Sequence)]
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

pub const DATE_FORMATS: &'static [(&'static str, &'static str)] = &[
    ("YYYY/MM/DD", "%Y/%m/%d"),
    ("MM/DD/YYYY", "%m/%d/%Y"),
    ("DD/MM/YYYY", "%d/%m/%Y"),
    ("YYYY-MM-DD", "%Y-%m-%d"),
    ("MM-DD-YYYY", "%m-%d-%Y"),
    ("DD-MM-YYYY", "%d-%m-%Y"),
    ("YYYY MM DD", "%Y %m %d"),
    ("MM DD YYYY", "%m %d %Y"),
    ("DD MM YYYY", "%d %m %Y"),
    ("YYYYMMDD", "%Y%m%d"),
    ("MMDDYYYY", "%m%d%Y"),
    ("DDMMYYYY", "%d%m%Y"),
    ("YY/MM/DD", "%y/%m/%d"),
    ("MM/DD/YY", "%m/%d/%y"),
    ("DD/MM/YY", "%d/%m/%y"),
    ("YY-MM-DD", "%y-%m-%d"),
    ("MM-DD-YY", "%m-%d-%y"),
    ("DD-MM-YY", "%d-%m-%y"),
    ("YY MM DD", "%y %m %d"),
    ("MM DD YY", "%m %d %y"),
    ("DD MM YY", "%d %m %y"),
    ("YYMMDD", "%y%m%d"),
    ("MMDDYY", "%m%d%y"),
    ("DDMMYY", "%d%m%y"),
];

pub struct PendingUpload {
    headers: Vec<String>,
    cells: Vec<String>,
    row_count: usize,
}

impl PendingUpload {
    pub fn new(headers: Vec<String>, cells: Vec<String>, row_count: usize) -> PendingUpload {
        PendingUpload {
            headers,
            cells,
            row_count,
        }
    }

    pub fn get_rows(&self, row_index: usize, row_count: usize) -> crate::error::Result<&[String]> {
        if row_index > self.row_count {
            return Err(MoneyError::RowIndex(row_index));
        } else if (row_index + row_count) > self.row_count {
            return Err(MoneyError::RowIndex(row_index + row_count));
        }

        let start = self.headers.len() * row_index;
        let end = self.headers.len() * (row_index + row_count);

        Ok(&self.cells[start..end])
    }

    pub fn try_submit(
        &self,
        header_selections: &[HeaderOption],
        date_format: usize,
    ) -> crate::error::Result<SubmitResult> {
        if header_selections.len() != self.headers.len() {
            return Ok(SubmitResult::HeaderError(String::from(
                "Header selection count differs from header count",
            )));
        }

        let header_selections = match HeaderSelections::from(header_selections) {
            Ok(s) => s,
            Err(e) => return Ok(e),
        };

        if date_format >= DATE_FORMATS.len() {
            return Err(MoneyError::InvalidDateFormat);
        }

        let format_str = DATE_FORMATS[date_format].1;

        for row_index in 0..self.row_count {
            let row = self.get_rows(row_index, 1)?;

            let date_str = &row[header_selections.date_col as usize];
            let name_str = &row[header_selections.name_col as usize];
            let desc_str = &row[header_selections.desc_col as usize];
            let amount_str = &row[header_selections.amount_col as usize];

            let date = match NaiveDate::parse_from_str(date_str, format_str) {
                Ok(d) => d,
                Err(_) => {
                    return Ok(SubmitResult::CellError {
                        row: row_index,
                        col: header_selections.date_col as usize,
                        msg: format!("Cell \"{}\" could not be parsed as a date", date_str),
                    })
                }
            };

            let amount = match amount_str.parse::<f32>() {
                Ok(a) => a,
                Err(_) => {
                    return Ok(SubmitResult::CellError {
                        row: row_index,
                        col: header_selections.amount_col as usize,
                        msg: format!("Cell \"{}\" could not be parsed as an amount", amount_str),
                    })
                }
            };

            dbg!("Parse row", row_index, date, name_str, desc_str, amount);
        }

        Ok(SubmitResult::Success)
    }
}

pub struct HeaderSelections {
    pub date_col: i64,
    pub name_col: i64,
    pub desc_col: i64,
    pub amount_col: i64,
}

pub fn validate_headers(selections: &[HeaderOption]) -> Result<HeaderSelections> {
    let mut date_col = None;
    let mut name_col = None;
    let mut desc_col = None;
    let mut amount_col = None;

    for (idx, selection) in selections.iter().enumerate() {
        let col = match selection {
            HeaderOption::Date => &mut date_col,
            HeaderOption::Name => &mut name_col,
            HeaderOption::Description => &mut desc_col,
            HeaderOption::Amount => &mut amount_col,
            HeaderOption::Unused => continue,
        };

        if col.is_some() {
            return Err(anyhow!(
                "Duplicate header used: {0}",
                to_variant_name(selection).unwrap()
            ));
        };
        *col = Some(idx);
    }

    let mut missing_headers = Vec::new();
    if date_col.is_none() {
        missing_headers.push(HeaderOption::Date);
    }
    if name_col.is_none() {
        missing_headers.push(HeaderOption::Name);
    }
    if desc_col.is_none() {
        missing_headers.push(HeaderOption::Description);
    }
    if amount_col.is_none() {
        missing_headers.push(HeaderOption::Amount);
    }

    if missing_headers.len() > 0 {
        let mut header_strings = Vec::with_capacity(missing_headers.len());
        for header in missing_headers {
            header_strings.push(to_variant_name(&header).unwrap());
        }

        return Err(anyhow!(
            "Missing required headers: {}",
            header_strings.join(", ")
        ));
    }

    Ok(HeaderSelections {
        date_col: date_col.unwrap() as i64,
        name_col: name_col.unwrap() as i64,
        desc_col: desc_col.unwrap() as i64,
        amount_col: amount_col.unwrap() as i64,
    })
}

impl HeaderSelections {
    fn from(selections: &[HeaderOption]) -> std::result::Result<HeaderSelections, SubmitResult> {
        let mut date_col = None;
        let mut name_col = None;
        let mut desc_col = None;
        let mut amount_col = None;

        for (idx, selection) in selections.iter().enumerate() {
            let col = match selection {
                HeaderOption::Date => &mut date_col,
                HeaderOption::Name => &mut name_col,
                HeaderOption::Description => &mut desc_col,
                HeaderOption::Amount => &mut amount_col,
                HeaderOption::Unused => continue,
            };

            if col.is_some() {
                return Err(SubmitResult::HeaderError(format!(
                    "Duplicate header used: {0}",
                    to_variant_name(selection).unwrap()
                )));
            };
            *col = Some(idx);
        }

        let mut missing_headers = Vec::new();
        if date_col.is_none() {
            missing_headers.push(HeaderOption::Date);
        }
        if name_col.is_none() {
            missing_headers.push(HeaderOption::Name);
        }
        if desc_col.is_none() {
            missing_headers.push(HeaderOption::Description);
        }
        if amount_col.is_none() {
            missing_headers.push(HeaderOption::Amount);
        }

        if missing_headers.len() > 0 {
            let mut header_strings = Vec::with_capacity(missing_headers.len());
            for header in missing_headers {
                header_strings.push(to_variant_name(&header).unwrap());
            }

            return Err(SubmitResult::HeaderError(format!(
                "Missing required headers: {}",
                header_strings.join(", ")
            )));
        }

        Ok(HeaderSelections {
            date_col: date_col.unwrap() as i64,
            name_col: name_col.unwrap() as i64,
            desc_col: desc_col.unwrap() as i64,
            amount_col: amount_col.unwrap() as i64,
        })
    }
}

#[derive(PartialEq)]
pub enum SubmitResult {
    Success,
    HeaderError(String),
    CellError { row: usize, col: usize, msg: String },
}
