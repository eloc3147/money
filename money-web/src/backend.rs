use common::SubmitDataRequest;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

use crate::{MoneyError, MoneyErrorKind};

pub struct Backend;

impl Backend {
    pub async fn submit_data(
        headers: Vec<String>,
        rows: Vec<String>,
        width: usize,
    ) -> Result<(), JsValue> {
        let request_headers = Headers::new()?;
        request_headers.append("Content-Type", "application/json")?;

        let mut request_config = RequestInit::new();
        request_config.method("POST");
        request_config.mode(RequestMode::Cors);
        request_config.headers(&request_headers);

        let body: JsValue = serde_json::to_string(&SubmitDataRequest {
            headers,
            rows,
            width,
        })
        .map_err(|err| {
            JsValue::from(MoneyError {
                kind: MoneyErrorKind::EncodingError,
                msg: format!("Error encoding request: {}", err),
            })
        })?
        .into();

        request_config.body(Some(&body));

        let url = "/api/add_transactions";

        let request = Request::new_with_str_and_init(&url, &request_config)?;
        let window = web_sys::window().expect("Unable to load window");
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

        let _resp: Response = resp_value
            .dyn_into()
            .expect("Response value is not a Response");
        Ok(())
    }
}
