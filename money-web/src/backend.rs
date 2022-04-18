use std::marker::PhantomData;

use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{console, Headers, Request, RequestInit, RequestMode, Response};

use crate::{MoneyError, MoneyErrorKind};

use common::SubmitDataRequest;

struct BackendResponse<'a, D> {
    resp_text: String,
    phantom: PhantomData<&'a D>,
}

impl<'a, D> BackendResponse<'a, D>
where
    D: Deserialize<'a>,
{
    pub fn deserialize(&'a self) -> Result<D, MoneyError> {
        serde_json::from_str(&self.resp_text).map_err(|err| MoneyError {
            kind: MoneyErrorKind::EncodingError,
            msg: format!("Error encoding request: {}", err),
        })
    }
}

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

        if !resp.ok() {
            return Err(MoneyError {
                kind: MoneyErrorKind::RequestError,
                msg: format!(
                    "Request to {} gave status {} {}",
                    endpoint,
                    resp.status(),
                    resp.status_text()
                ),
            }
            .into());
        }

        Ok(resp)
    }

    async fn request_json<'a, S, D>(
        request: &S,
        endpoint: &str,
    ) -> Result<BackendResponse<'a, D>, JsValue>
    where
        S: Serialize,
        D: Deserialize<'a>,
        D: 'a,
    {
        let resp = Self::send_request(request, endpoint).await?;
        let resp_text = JsFuture::from(resp.text()?).await?.as_string().unwrap();
        Ok(BackendResponse {
            resp_text,
            phantom: PhantomData,
        })
    }

    pub async fn add_transactions(
        headers: Vec<String>,
        cells: Vec<String>,
        width: usize,
    ) -> Result<(), JsValue> {
        let request = SubmitDataRequest {
            headers,
            cells,
            width,
        };
        let resp = Self::send_request(&request, "/api/add_transactions").await?;
        console::log_2(&"Resp: ".into(), &resp);
        Ok(())
    }
}
