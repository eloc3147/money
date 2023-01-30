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
}
