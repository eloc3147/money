use serde::{Deserialize, Serialize};
use serde_variant::to_variant_name;

use crate::old::error::{MoneyError, Result};

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

    pub fn get_rows(&self, row_index: usize, row_count: usize) -> Result<&[String]> {
        if row_index > self.row_count {
            return Err(MoneyError::RowIndex(row_index));
        } else if (row_index + row_count) > self.row_count {
            return Err(MoneyError::RowIndex(row_index + row_count));
        }

        let start = self.headers.len() * row_index;
        let end = self.headers.len() * (row_index + row_count);

        Ok(&self.cells[start..end])
    }

    pub fn try_submit(&self, header_selections: &[HeaderOption]) -> Result<SubmitResult> {
        if header_selections.len() != self.headers.len() {
            return Ok(SubmitResult::HeaderError(String::from(
                "Header selection count differs from header count",
            )));
        }

        let header_selections = match HeaderSelections::from(header_selections) {
            Ok(s) => s,
            Err(e) => return Ok(e),
        };

        for row_index in 0..self.row_count {
            let row = self.get_rows(row_index, 1)?;

            let date_str = &row[header_selections.date_col];
            let name_str = &row[header_selections.name_col];
            let desc_str = &row[header_selections.desc_col];
            let amount_str = &row[header_selections.amount_col];

            let amount = match amount_str.parse::<f32>() {
                Ok(a) => a,
                Err(_) => {
                    return Ok(SubmitResult::CellError {
                        row: row_index,
                        col: header_selections.amount_col,
                        msg: format!("Cell \"{}\" could not be parsed as an amount", amount_str),
                    });
                }
            };

            dbg!("Parse row", row_index, date_str, name_str, desc_str, amount);
        }

        Ok(SubmitResult::Success)
    }
}

struct HeaderSelections {
    date_col: usize,
    name_col: usize,
    desc_col: usize,
    amount_col: usize,
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
            date_col: date_col.unwrap(),
            name_col: name_col.unwrap(),
            desc_col: desc_col.unwrap(),
            amount_col: amount_col.unwrap(),
        })
    }
}

#[derive(PartialEq)]
pub enum SubmitResult {
    Success,
    HeaderError(String),
    CellError { row: usize, col: usize, msg: String },
}
