use csv;
use js_sys::Array;
use std::io::Cursor;
use std::iter::{FromIterator, IntoIterator};
use std::ops::Range;
use wasm_bindgen::prelude::*;

use super::{MoneyError, MoneyErrorKind};
#[wasm_bindgen]
pub struct UploadSession {
    file: InputFile,
}

#[wasm_bindgen]
impl UploadSession {
    pub fn from_string(file: String) -> Result<UploadSession, JsValue> {
        let file = Self::parse_csv(file).map_err(|e| {
            MoneyError::new(MoneyErrorKind::FileLoadingError, format!("{}", e).into())
        })?;

        Ok(UploadSession { file })
    }

    #[wasm_bindgen]
    pub fn get_row_count(&self) -> JsValue {
        JsValue::from_f64(self.file.rows.len() as f64)
    }

    #[wasm_bindgen]
    pub fn get_headers(&self) -> Array {
        Array::from_iter(self.file.headers().iter().map(|s| JsValue::from_str(s)))
    }

    #[wasm_bindgen]
    pub fn get_row_slice(&self, index: usize, length: usize) -> Result<Array, JsValue> {
        if index + length > self.file.height() {
            return Err(MoneyError::new(
                MoneyErrorKind::OutOfBounds,
                format!("Index {} plus length {} goes out of bounds", index, length).into(),
            )
            .into());
        }

        let rows = Array::from_iter(
            self.file
                .iter_rows(index..(index + length))?
                .map(|r| Array::from_iter(r.iter().map(|s| JsValue::from_str(s)))),
        );

        Ok(rows)
    }

    fn parse_csv(file: String) -> Result<InputFile, csv::Error> {
        let mut reader = csv::Reader::from_reader(Cursor::new(file));

        let mut input_file = InputFile::from_headers(reader.headers()?.iter());

        for row in reader.records() {
            input_file.push_row(row?.iter());
        }

        Ok(input_file)
    }
}

struct InputFile {
    headers: Vec<String>,
    rows: Vec<String>,
    width: usize,
}

impl InputFile {
    pub fn new(width: usize) -> InputFile {
        InputFile {
            headers: Vec::new(),
            rows: Vec::new(),
            width,
        }
    }

    pub fn from_headers<H>(headers: H) -> InputFile
    where
        H: IntoIterator,
        H::Item: AsRef<str>,
    {
        let headers: Vec<String> = headers.into_iter().map(|s| s.as_ref().to_owned()).collect();
        let width = headers.len();
        let rows = Vec::new();

        InputFile {
            headers,
            rows,
            width,
        }
    }

    pub fn set_headers<H>(&mut self, headers: H) -> Result<(), MoneyError>
    where
        H: IntoIterator,
        H::Item: AsRef<str>,
    {
        self.headers.reserve(self.width);

        let mut counter = 0usize;
        for cell in headers.into_iter() {
            counter += 1;

            if counter > self.width {
                break;
            }

            self.headers.push(cell.as_ref().to_owned());
        }

        if counter != self.width {
            self.headers.clear();

            return Err(MoneyError::new(
                MoneyErrorKind::RowWidthMismatch,
                format!("Header had length {}, expected {}.", counter, self.width),
            ));
        }

        Ok(())
    }

    pub fn push_row<R>(&mut self, row: R) -> Result<(), MoneyError>
    where
        R: IntoIterator,
        R::Item: AsRef<str>,
    {
        self.rows.reserve(self.width);
        let starting_len = self.rows.len();

        let mut counter = 0usize;
        for cell in row.into_iter() {
            counter += 1;

            if counter > self.width {
                break;
            }

            self.rows.push(cell.as_ref().to_owned());
        }

        if counter != self.width {
            self.rows.truncate(starting_len);

            return Err(MoneyError::new(
                MoneyErrorKind::RowWidthMismatch,
                format!("Row had length {}, expected {}.", counter, self.width),
            ));
        }

        Ok(())
    }

    pub fn get_row(&self, index: usize) -> Option<&[String]> {
        if index >= self.height() {
            return None;
        }

        // Take a slice one row's width in len
        Some(&self.rows[(index * self.width)..((index + 1) * self.width)])
    }

    pub fn iter_rows(&self, index: Range<usize>) -> Result<RowsIter, MoneyError> {
        if index.end > self.height() {
            return Err(MoneyError::new(
                MoneyErrorKind::OutOfBounds,
                "The selected index is out of bounds".into(),
            ));
        }

        Ok(RowsIter {
            curr: index.start,
            range: index,
            file: &self,
        })
    }

    pub fn headers(&self) -> &[String] {
        &self.headers[..]
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.rows.len() / self.width
    }
}

pub struct RowsIter<'f> {
    curr: usize,
    range: Range<usize>,
    file: &'f InputFile,
}

impl<'f> Iterator for RowsIter<'f> {
    type Item = &'f [String];

    // Here, we define the sequence using `.curr` and `.next`.
    // The return type is `Option<T>`:
    //     * When the `Iterator` is finished, `None` is returned.
    //     * Otherwise, the next value is wrapped in `Some` and returned.
    fn next(&mut self) -> Option<&'f [String]> {
        if self.curr >= self.range.end {
            return None;
        }

        let item = self.file.get_row(self.curr);
        self.curr += 1;
        item
    }
}
