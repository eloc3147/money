use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct SubmitDataRequest {
    pub headers: Vec<String>,
    pub cells: Vec<String>,
    pub width: usize,
}
