use csv_async::{self, AsyncReader};
use rocket::{
    Route, State,
    data::{Data, DataStream, ToByteUnit},
    futures::StreamExt,
    serde::{Serialize, json::Json},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::old::api::{MoneyMsg, MoneyResult};
use crate::old::backend::{BackendHandle, HeaderOption, SubmitResult};
use crate::old::error::Result;

struct CsvFile {
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
struct AddUploadResponse {
    upload_id: Uuid,
    headers: Vec<String>,
    header_suggestions: Vec<HeaderOption>,
    row_count: usize,
}

#[post("/", data = "<file>")]
async fn add_upload(b: &State<BackendHandle>, file: Data<'_>) -> MoneyResult<AddUploadResponse> {
    let file_stream = file.open(100u8.mebibytes());
    let parsed = parse_csv(file_stream).await?;

    let upload_id = {
        let mut guard = b.lock().await;
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
struct GetUploadRowsResponse {
    cells: Vec<String>,
}

#[get("/<upload_id>/rows?<row_index>&<row_count>")]
async fn list_upload_rows(
    b: &State<BackendHandle>,
    upload_id: &str,
    row_index: usize,
    row_count: usize,
) -> MoneyResult<GetUploadRowsResponse> {
    let uuid = Uuid::parse_str(upload_id)?;
    let cells = {
        let guard = b.lock().await;
        guard.get_pending_upload_rows(uuid, row_index, row_count)?
    };
    Ok(MoneyMsg::new(GetUploadRowsResponse { cells }))
}

#[derive(Debug, Deserialize)]
struct SubmitUploadRequest {
    header_selections: Vec<HeaderOption>,
}

#[derive(Clone, PartialEq, Serialize)]
struct SubmitUploadResponse {
    status: &'static str,
    header_error: Option<String>,
    row: Option<usize>,
    col: Option<usize>,
    cell_error: Option<String>,
}

#[post("/<upload_id>/submit", data = "<data>")]
async fn submit_upload(
    b: &State<BackendHandle>,
    upload_id: &str,
    data: Json<SubmitUploadRequest>,
) -> MoneyResult<SubmitUploadResponse> {
    let uuid = Uuid::parse_str(upload_id)?;

    let submit_result = {
        let guard = b.lock().await;
        guard.try_submit_upload(uuid, &data.header_selections)?
    };

    let resp = match submit_result {
        SubmitResult::Success => SubmitUploadResponse {
            status: "success",
            header_error: None,
            row: None,
            col: None,
            cell_error: None,
        },
        SubmitResult::HeaderError(e) => SubmitUploadResponse {
            status: "header_error",
            header_error: Some(e),
            row: None,
            col: None,
            cell_error: None,
        },
        SubmitResult::CellError { row, col, msg } => SubmitUploadResponse {
            status: "cell_error",
            header_error: None,
            row: Some(row),
            col: Some(col),
            cell_error: Some(msg),
        },
    };

    Ok(MoneyMsg::new(resp))
}

pub fn routes() -> Vec<Route> {
    routes![add_upload, list_upload_rows, submit_upload]
}
