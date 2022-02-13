use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct SubmitDataRequest {
    pub headers: Vec<String>,
    pub rows: Vec<String>,
    pub width: usize,
}
