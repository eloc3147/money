use csv;
use enum_iterator::IntoEnumIterator;
use js_sys::{Array, JsString, Map};
use std::io::Cursor;
use std::iter::IntoIterator;
use std::ops::Range;
use wasm_bindgen::prelude::*;

use super::{MoneyError, MoneyErrorKind};

use crate::backend::Backend;

const REQUIRED_FIELDS: &[HeaderOption] = &[
    HeaderOption::Date,
    HeaderOption::Description,
    HeaderOption::Amount,
];

// Hack because there's no official repitition count in std
macro_rules! replace_expr {
    ($_t:tt $sub:expr) => {
        $sub
    };
}

macro_rules! js_array {
    [$($item:expr),+] => {
        #[allow(unused_assignments)]
        {
            let size = <[()]>::len(&[$(replace_expr!(($item) ())),*]);
            let array = Array::new_with_length(size as u32);
            let mut idx = 0u32;
            $(
                array.set(idx, $item);
                idx += 1u32;
            )*
            array
        }
    }
}

#[derive(Clone, IntoEnumIterator, PartialEq, Copy)]
enum HeaderOption {
    Unused,
    Date,
    Name,
    Description,
    Amount,
}

impl HeaderOption {
    fn as_str(&self) -> &'static str {
        match self {
            HeaderOption::Unused => "-",
            HeaderOption::Date => "Date",
            HeaderOption::Name => "Name",
            HeaderOption::Description => "Description",
            HeaderOption::Amount => "Amount",
        }
    }

    fn from_str(string: &str) -> HeaderOption {
        match string.trim().to_lowercase().as_ref() {
            "date" => HeaderOption::Date,
            "name" => HeaderOption::Name,
            "memo" | "description" => HeaderOption::Description,
            "amount" => HeaderOption::Amount,
            _ => HeaderOption::Unused,
        }
    }
}

#[wasm_bindgen]
pub struct UploadSession {
    file: InputFile,
    header_selections: Vec<HeaderOption>,
}

#[wasm_bindgen]
impl UploadSession {
    pub fn from_string(file: String) -> Result<UploadSession, MoneyError> {
        let file = Self::parse_csv(file)?;
        let header_selections = file.header_suggestions().collect();
        Ok(UploadSession {
            file,
            header_selections,
        })
    }

    #[wasm_bindgen]
    pub fn get_row_count(&self) -> JsValue {
        JsValue::from_f64(self.file.row_count() as f64)
    }

    #[wasm_bindgen]
    pub fn get_headers(&self) -> Array {
        self.file
            .headers()
            .iter()
            .map(|s| JsValue::from_str(s))
            .collect()
    }

    #[wasm_bindgen]
    pub fn get_header_suggestions(&self) -> Array {
        let cell_val_key = JsValue::from_str("cell_value");
        let cell_selected_key = JsValue::from_str("selected");

        let header_mapping: Vec<(HeaderOption, JsString)> = HeaderOption::into_enum_iter()
            .map(|option| (option, JsString::from(option.as_str())))
            .collect();

        self.file
            .header_suggestions()
            .map(|suggestion| {
                header_mapping
                    .iter()
                    .map(|(option, option_str)| {
                        let map = Map::new();
                        map.set(&cell_val_key, &option_str);
                        map.set(&cell_selected_key, &JsValue::from(suggestion == *option));
                        map
                    })
                    .collect::<Array>()
            })
            .collect()
    }

    #[wasm_bindgen]
    pub fn get_row_slice(&self, index: usize, length: usize) -> Result<Array, MoneyError> {
        let height = self.file.row_count();
        if index + length > height {
            return Err(MoneyError::new(
                MoneyErrorKind::OutOfBounds,
                format!(
                    "Index {} plus length {} goes past length {}",
                    index, length, height
                ),
            )
            .into());
        }

        let rows = self
            .file
            .iter_rows(index..(index + length))?
            .map(|r| r.iter().map(|s| JsValue::from_str(s)).collect::<Array>())
            .collect();

        Ok(rows)
    }

    #[wasm_bindgen]
    pub fn update_header_selection(
        &mut self,
        column_index: usize,
        selection: String,
    ) -> Result<(), MoneyError> {
        if column_index > self.header_selections.len() {
            return Err(MoneyError::new(
                MoneyErrorKind::OutOfBounds,
                format!(
                    "Column index {} greater than length {}",
                    column_index,
                    self.header_selections.len()
                ),
            )
            .into());
        }

        self.header_selections[column_index] = HeaderOption::from_str(&selection);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_selection_error(&self) -> Option<String> {
        let missing_fields: Vec<&str> = REQUIRED_FIELDS
            .iter()
            .filter_map(|field| {
                if !self.header_selections.contains(&field) {
                    return Some(field.as_str());
                }
                None
            })
            .collect();

        if missing_fields.len() > 0 {
            return Some(format!("Missing required fields: {}.", missing_fields.join(", ")).into());
        }

        let mut used_fields: Vec<HeaderOption> = self
            .header_selections
            .iter()
            .filter_map(|o| {
                if *o != HeaderOption::Unused {
                    return Some(*o);
                }
                None
            })
            .collect();

        let (_, duplicates) = used_fields.partition_dedup();
        if duplicates.len() > 0 {
            let dup_strings: Vec<&str> = duplicates.iter().map(|o| o.as_str()).collect();
            return Some(format!("Duplicated fields: {}.", dup_strings.join(", ")).into());
        }

        None
    }

    #[wasm_bindgen]
    pub async fn submit_data(self) -> Result<(), JsValue> {
        Backend::add_transactions(self.file.headers, self.file.cells, self.file.width).await
    }

    fn parse_csv(file: String) -> Result<InputFile, MoneyError> {
        let mut reader = csv::Reader::from_reader(Cursor::new(file));

        let mut input_file = InputFile::from_headers(reader.headers()?.iter());

        for row in reader.records() {
            input_file.push_row(row?.iter())?;
        }

        Ok(input_file)
    }
}

struct InputFile {
    headers: Vec<String>,
    cells: Vec<String>,
    width: usize,
}

impl InputFile {
    pub fn new(width: usize) -> InputFile {
        InputFile {
            headers: Vec::with_capacity(width),
            cells: Vec::new(),
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
        let cells = Vec::new();

        InputFile {
            headers,
            cells,
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
        self.cells.reserve(self.width);
        let starting_len = self.cells.len();

        let mut counter = 0usize;
        for cell in row.into_iter() {
            counter += 1;

            if counter > self.width {
                break;
            }

            self.cells.push(cell.as_ref().to_owned());
        }

        if counter != self.width {
            self.cells.truncate(starting_len);

            return Err(MoneyError::new(
                MoneyErrorKind::RowWidthMismatch,
                format!("Row had length {}, expected {}.", counter, self.width),
            ));
        }

        Ok(())
    }

    pub fn get_row(&self, index: usize) -> Option<&[String]> {
        if index >= self.row_count() {
            return None;
        }

        // Take a slice one row's width in len
        Some(&self.cells[(index * self.width)..((index + 1) * self.width)])
    }

    pub fn iter_rows(&self, index: Range<usize>) -> Result<RowsIter, MoneyError> {
        if index.end > self.row_count() {
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

    pub fn header_suggestions(&self) -> impl Iterator<Item = HeaderOption> + '_ {
        self.headers.iter().map(|h| HeaderOption::from_str(&h))
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn row_count(&self) -> usize {
        self.cells.len() / self.width
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
