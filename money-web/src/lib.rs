#![feature(slice_partition_dedup)]

mod uploader;
mod utils;

use wasm_bindgen::prelude::*;
use web_sys::FileReader;

use uploader::UploadSession;
use utils::set_panic_hook;

#[wasm_bindgen]
pub struct Money {}

#[wasm_bindgen]
impl Money {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Money {
        set_panic_hook();
        Money {}
    }

    #[wasm_bindgen]
    pub fn load_file(self, file_reader: FileReader) -> Result<UploadSession, JsValue> {
        file_reader
            .result()
            .map_err(|e| {
                MoneyError::new(
                    MoneyErrorKind::FileLoadingError,
                    format!("error reading file: {:#?}", e),
                )
            })?
            .as_string()
            .ok_or_else(|| MoneyError::new(MoneyErrorKind::FileLoadingError, "not a text".into()))
            .map(|s| UploadSession::from_string(s).map_err(|e| e))?
    }
}

#[wasm_bindgen]
pub struct MoneyError {
    kind: MoneyErrorKind,
    msg: String,
}

impl MoneyError {
    fn new(kind: MoneyErrorKind, msg: String) -> MoneyError {
        MoneyError { kind, msg }
    }
}

impl From<csv::Error> for MoneyError {
    fn from(error: csv::Error) -> MoneyError {
        MoneyError {
            kind: MoneyErrorKind::FileLoadingError,
            msg: format!("{:?}", error).into(),
        }
    }
}

#[wasm_bindgen]
pub enum MoneyErrorKind {
    FileLoadingError,
    OutOfBounds,
    RowWidthMismatch,
    UnexpectedFailure,
}
