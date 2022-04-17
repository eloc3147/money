use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

use crate::{MoneyError, MoneyErrorKind};

use common::SubmitDataRequest;

pub struct Backend;

impl Backend {
    async fn send_request<S>(request: &S, endpoint: &str) -> Result<Response, JsValue>
    where
        S: Serialize,
    {
        let request_headers = Headers::new()?;
        request_headers.append("Content-Type", "application/json")?;

        let mut request_config = RequestInit::new();
        request_config.method("POST");
        request_config.mode(RequestMode::Cors);
        request_config.headers(&request_headers);

        let body: JsValue = serde_json::to_string(request)
            .map_err(|err| {
                JsValue::from(MoneyError {
                    kind: MoneyErrorKind::EncodingError,
                    msg: format!("Error encoding request: {}", err),
                })
            })?
            .into();

        request_config.body(Some(&body));

        let request = Request::new_with_str_and_init(&endpoint, &request_config)?;
        let window = web_sys::window().expect("Unable to load window");
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

        let resp: Response = resp_value
            .dyn_into()
            .expect("Response value is not a Response");
        Ok(resp)
    }

    pub async fn add_transactions(
        headers: Vec<String>,
        rows: Vec<String>,
        width: usize,
    ) -> Result<(), JsValue> {
        let request = SubmitDataRequest {
            headers,
            rows,
            width,
        };
        let resp = Self::send_request(&request, "/api/add_transactions").await?;
        Ok(())
    }
}
