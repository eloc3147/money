mod uploader;
mod utils;

use wasm_bindgen::prelude::*;
use web_sys::FileReader;

use uploader::UploadSession;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct Money {}

#[wasm_bindgen]
impl Money {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Money {
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

#[wasm_bindgen]
pub enum MoneyErrorKind {
    FileLoadingError,
    OutOfBounds,
    RowWidthMismatch,
}
