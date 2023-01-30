use serde_variant::to_variant_name;

use crate::components::HeaderOption;
use crate::error::{MoneyError, Result};

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
        let mut date_col = None;
        let mut name_col = None;
        let mut desc_col = None;
        let mut amount_col = None;

        for (idx, selection) in header_selections.iter().enumerate() {
            let col = match selection {
                HeaderOption::Date => &mut date_col,
                HeaderOption::Name => &mut name_col,
                HeaderOption::Description => &mut desc_col,
                HeaderOption::Amount => &mut amount_col,
                HeaderOption::Unused => continue,
            };

            if col.is_some() {
                return Ok(SubmitResult::DuplicateHeader(*selection));
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
            return Ok(SubmitResult::MissingRequiredHeaders(missing_headers));
        }

        dbg!("Upload success");
        dbg!(date_col);
        dbg!(name_col);
        dbg!(desc_col);
        dbg!(amount_col);

        Ok(SubmitResult::Success)
    }
}

#[derive(PartialEq)]
pub enum SubmitResult {
    Success,
    MissingRequiredHeaders(Vec<HeaderOption>),
    DuplicateHeader(HeaderOption),
}

impl SubmitResult {
    pub fn to_string(&self) -> String {
        match self {
            SubmitResult::Success => String::new(),
            SubmitResult::MissingRequiredHeaders(missing_headers) => {
                let mut header_strings = Vec::with_capacity(missing_headers.len());
                for header in missing_headers {
                    header_strings.push(to_variant_name(header).unwrap());
                }

                format!("Missing required headers: {}", header_strings.join(", "))
            }
            SubmitResult::DuplicateHeader(header) => format!(
                "Duplicate header used: {0}",
                to_variant_name(header).unwrap()
            ),
        }
    }
}
