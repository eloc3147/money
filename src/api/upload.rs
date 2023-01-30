use csv_async::{self, AsyncReader};
use rocket::{
    data::{Data, DataStream, ToByteUnit},
    futures::StreamExt,
    serde::{json::Json, Serialize},
    Route, State,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::components::{HeaderOption, MoneyMsg, MoneyResult};
use crate::data_store::SharedDataStore;
use crate::error::Result;

pub struct CsvFile {
    headers: Vec<String>,
    cells: Vec<String>,
    row_count: usize,
}

async fn parse_csv(stream: DataStream<'_>) -> Result<CsvFile> {
    let mut reader = AsyncReader::from_reader(stream);

    let headers: Vec<String> = reader.headers().await?.iter().map(str::to_string).collect();
    let mut cells = Vec::new();

    let mut row_count = 0usize;
    let mut records = reader.records();
    while let Some(row) = records.next().await {
        for cell in row?.iter() {
            cells.push(cell.to_string());
        }
        row_count += 1;
    }

    Ok(CsvFile {
        headers,
        cells,
        row_count,
    })
}

#[derive(Clone, PartialEq, Serialize)]
pub struct AddUploadResponse {
    upload_id: Uuid,
    headers: Vec<String>,
    header_suggestions: Vec<HeaderOption>,
    row_count: usize,
}

#[post("/", data = "<file>")]
async fn add_upload(ds: &State<SharedDataStore>, file: Data<'_>) -> MoneyResult<AddUploadResponse> {
    let file_stream = file.open(100u8.mebibytes());
    let parsed = parse_csv(file_stream).await?;

    let upload_id = {
        let mut guard = ds.lock().await;
        guard.add_pending_upload(parsed.headers.clone(), parsed.cells, parsed.row_count)
    };

    let header_suggestions = parsed
        .headers
        .iter()
        .map(|h| HeaderOption::get_header_suggestion(h))
        .collect();

    Ok(MoneyMsg::new(AddUploadResponse {
        upload_id,
        headers: parsed.headers,
        header_suggestions,
        row_count: parsed.row_count,
    }))
}

#[derive(Clone, PartialEq, Serialize)]
pub struct GetUploadRowsResponse {
    cells: Vec<String>,
}

#[get("/<upload_id>/rows?<row_index>&<row_count>")]
pub async fn list_upload_rows(
    ds: &State<SharedDataStore>,
    upload_id: &str,
    row_index: usize,
    row_count: usize,
) -> MoneyResult<GetUploadRowsResponse> {
    let uuid = Uuid::parse_str(upload_id)?;
    let cells = {
        let guard = ds.lock().await;
        guard.get_pending_upload_rows(uuid, row_index, row_count)?
    };
    Ok(MoneyMsg::new(GetUploadRowsResponse { cells }))
}

#[derive(Debug, Deserialize)]
pub struct SubmitUploadRequest {
    header_selections: Vec<HeaderOption>,
}

#[post("/<upload_id>/submit", data = "<data>")]
pub async fn submit_upload(upload_id: &str, data: Json<SubmitUploadRequest>) -> MoneyResult<()> {
    let uuid = Uuid::parse_str(upload_id)?;

    println!("Upload submitted with selections: {:?}", data);

    Ok(MoneyMsg::new(()))
}

pub fn routes() -> Vec<Route> {
    routes![add_upload, list_upload_rows, submit_upload]
}
