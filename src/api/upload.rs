use anyhow::{anyhow, bail, Context};
use chrono::NaiveDate;
use csv_async::{self, AsyncReader};
use enum_iterator::{all, cardinality};
use rocket::{
    data::{Data, ToByteUnit},
    futures::{StreamExt, TryStreamExt},
    serde::json::Json,
    Route,
};
use rocket_db_pools::sqlx::{self, Executor, Row, Statement};
use serde::{Deserialize, Serialize};
use serde_variant::to_variant_name;

use crate::{
    api::{ApiResponse, ApiResult},
    backend::{db::Db, upload::validate_headers, HeaderOption, DATE_FORMATS},
};

#[derive(Clone, PartialEq, Serialize)]
struct AddUploadResponse {
    upload_id: i64,
    headers: Vec<String>,
    header_suggestions: Vec<HeaderOption>,
    row_count: usize,
}

#[post("/", data = "<file>")]
async fn add_upload(db: &Db, file: Data<'_>) -> ApiResult<AddUploadResponse> {
    // Open CSV decoder
    let mut reader = AsyncReader::from_reader(file.open(100u8.mebibytes()));

    // Start DB transaction
    let mut transaction = db
        .begin()
        .await
        .context("Failed to start database request")?;

    let upload_id: i64 = sqlx::query(
        "INSERT INTO pending_uploads (column_count, row_count) VALUES (0, 0) RETURNING id;",
    )
    .fetch_one(&mut *transaction)
    .await?
    .try_get(0usize)?;

    // Prepare statement that needs significant re-use
    let insert = (&mut *transaction).prepare(
        "INSERT INTO pending_upload_cells (upload, header, row, column, value) VALUES (?, ?, ?, ?, ?);"
    ).await?;

    let mut headers = Vec::new();
    let mut header_suggestions = Vec::new();
    for (column, header) in reader
        .headers()
        .await
        .context("Error reading CSV headers")?
        .iter()
        .enumerate()
    {
        insert
            .query()
            .bind(upload_id)
            .bind(true)
            .bind(-1)
            .bind(column as i64)
            .bind(header)
            .execute(&mut *transaction)
            .await?;

        header_suggestions.push(HeaderOption::get_header_suggestion(header));
        headers.push(header.to_string())
    }

    let mut row_count = 0;
    let mut records = reader.records();
    while let Some(row) = records.next().await {
        for (column, cell) in row.context("Error reading CSV row")?.iter().enumerate() {
            insert
                .query()
                .bind(upload_id)
                .bind(false)
                .bind(row_count as i64)
                .bind(column as i64)
                .bind(cell)
                .execute(&mut *transaction)
                .await?;
        }

        row_count += 1;
    }

    sqlx::query("UPDATE pending_uploads SET column_count = ?, row_count = ? WHERE id = ?;")
        .bind(headers.len() as i64)
        .bind(row_count as i64)
        .bind(upload_id)
        .execute(&mut *transaction)
        .await?;

    transaction.commit().await?;

    Ok(ApiResponse::new(AddUploadResponse {
        upload_id,
        headers: headers,
        header_suggestions,
        row_count,
    }))
}

#[derive(Clone, PartialEq, Serialize)]
struct GetUploadRowsResponse {
    cells: Vec<String>,
}

#[get("/<upload_id>/rows?<row_index>&<row_count>")]
async fn list_upload_rows(
    db: &Db,
    upload_id: u64,
    row_index: usize,
    row_count: usize,
) -> ApiResult<GetUploadRowsResponse> {
    let mut cells_iter = sqlx::query(concat!(
        "SELECT (value) from pending_upload_cells",
        " WHERE upload = ? AND header = 0 AND row >= ? AND row < ?",
        " ORDER BY row ASC, column ASC;"
    ))
    .bind(upload_id as i64)
    .bind(row_index as i64)
    .bind((row_index + row_count) as i64)
    .fetch(&**db);

    let mut cells = Vec::new();
    while let Some(row) = cells_iter.try_next().await? {
        cells.push(row.try_get::<String, usize>(0)?);
    }

    Ok(ApiResponse::new(GetUploadRowsResponse { cells }))
}

#[derive(Debug, Deserialize)]
struct SubmitUploadRequest {
    header_selections: Vec<HeaderOption>,
    date_format: usize,
}

#[post("/<upload_id>/submit", data = "<data>")]
async fn submit_upload(
    db_reader: &Db,
    db_writer: &Db,
    upload_id: u64,
    data: Json<SubmitUploadRequest>,
) -> ApiResult<()> {
    // Count headers
    let mut header_iter =
        sqlx::query("SELECT (column) FROM pending_upload_cells WHERE upload = ? AND header = 1;")
            .bind(upload_id as i64)
            .fetch(&**db_reader);

    let mut header_count = 0;
    while let Some(_) = header_iter.try_next().await? {
        header_count += 1;
    }

    if data.header_selections.len() != header_count {
        return Err(anyhow!("Header selection count differs from header count").into());
    }

    let header_selections = validate_headers(&data.header_selections)?;

    if data.date_format >= DATE_FORMATS.len() {
        return Err(anyhow!("Invalid date format: {}", data.date_format).into());
    }

    let format_str = DATE_FORMATS[data.date_format].1;

    let mut transaction = db_writer
        .begin()
        .await
        .context("Failed to start database request")?;

    let mut cells_iter = sqlx::query_as(concat!(
        "SELECT (row, column, value) from pending_upload_cells",
        " WHERE upload = ? AND header = 0",
        " ORDER BY row ASC, column ASC;"
    ))
    .bind(upload_id as i64)
    .fetch(&mut *transaction);

    let mut current_row = 0;

    let mut date = None;
    let mut name = None;
    let mut desc = None;
    let mut amount = None;
    while let Some((r, c, v)) = cells_iter.try_next().await? {
        let row: i64 = r;
        let col: i64 = c;
        let value: String = v;

        if row > current_row {
            if let (Some(date_v), Some(name_v), Some(desc_v), Some(amount_v)) =
                (date, &name, &desc, amount)
            {
                info!(
                    "Row {}: Date: {:?}, Name: {}, Description: {}, Amount: {}",
                    current_row, date_v, name_v, desc_v, amount_v
                );
            } else if date.is_none() {
                return Err(anyhow!("Date missing for row {}", current_row).into());
            } else if name.is_none() {
                return Err(anyhow!("Name missing for row {}", current_row).into());
            } else if desc.is_none() {
                return Err(anyhow!("Description missing for row {}", current_row).into());
            } else if amount.is_none() {
                return Err(anyhow!("Amount missing for row {}", current_row).into());
            }

            date = None;
            name = None;
            desc = None;
            amount = None;
            current_row = row;
        }

        if col == header_selections.date_col as i64 {
            date = Some(
                NaiveDate::parse_from_str(&value, format_str).context(format!(
                    "Row {} Column {}: \"{}\" could not be parsed as a date",
                    row, col, value
                ))?,
            );
        } else if col == header_selections.name_col {
            name = Some(value.to_string());
        } else if col == header_selections.desc_col {
            desc = Some(value.to_string());
        } else if col == header_selections.amount_col {
            amount = Some(value.parse::<f32>().context(format!(
                "Row {} Column {}: \"{}\" could not be parsed as an amount",
                row, col, value
            ))?);
        }
    }

    Ok(ApiResponse::new(()))
}

#[derive(Clone, PartialEq, Serialize)]
pub struct GetUploadOptionsResponse {
    header_options: Vec<&'static str>,
    date_formats: Vec<&'static str>,
}

#[get("/options")]
pub async fn get_upload_options() -> ApiResult<GetUploadOptionsResponse> {
    let mut header_options = Vec::with_capacity(cardinality::<HeaderOption>());
    for option in all::<HeaderOption>() {
        header_options.push(to_variant_name(&option).unwrap());
    }

    let mut date_formats = Vec::with_capacity(DATE_FORMATS.len());
    for (format, _) in DATE_FORMATS {
        date_formats.push(*format);
    }

    Ok(ApiResponse::new(GetUploadOptionsResponse {
        header_options,
        date_formats,
    }))
}

pub fn routes() -> Vec<Route> {
    routes![get_upload_options, add_upload, list_upload_rows, submit_upload,]
}
