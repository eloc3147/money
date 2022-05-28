use csv_async::{self, AsyncReader};
use diesel::{prelude::*, Connection, RunQueryDsl};
use rocket::{
    data::{Data, DataStream, ToByteUnit},
    futures::StreamExt,
    serde::Serialize,
    Route,
};
use uuid::Uuid;

use crate::models::{Upload, UploadCell};
use crate::Db;
use crate::{
    components::{HeaderOption, MoneyMsg, MoneyResult},
    error::Result,
    models::{UploadCellInsert, UploadInsert},
};

async fn parse_csv(
    stream: DataStream<'_>,
    upload_id: i32,
) -> Result<(Vec<String>, Vec<UploadCellInsert>, usize)> {
    let mut reader = AsyncReader::from_reader(stream);
    let mut headers = Vec::new();
    let mut cells = Vec::new();

    for (column_num, cell) in reader.headers().await?.iter().enumerate() {
        headers.push(cell.to_string());
        cells.push(UploadCellInsert {
            upload_id,
            header: true,
            row_num: 0,
            column_num: column_num as i64,
            contents: cell.to_string(),
        });
    }

    let mut records = reader.records().enumerate();

    let mut row_count: usize = 0;
    while let Some((row_num, row)) = records.next().await {
        for (column_num, cell) in row?.iter().enumerate() {
            cells.push(UploadCellInsert {
                upload_id,
                header: false,
                row_num: row_num as i64,
                column_num: column_num as i64,
                contents: cell.to_string(),
            });
        }

        row_count += 1;
    }

    Ok((headers, cells, row_count))
}

#[derive(Clone, PartialEq, Serialize)]
pub struct AddUploadResponse {
    upload_id: Uuid,
    headers: Vec<String>,
    header_suggestions: Vec<HeaderOption>,
    row_count: usize,
}

#[post("/", data = "<file>")]
async fn add_upload(db: Db, file: Data<'_>) -> MoneyResult<AddUploadResponse> {
    let file_stream = file.open(10u8.mebibytes());

    let (upload_id, web_id) = db
        .run(
            move |conn| -> std::result::Result<(i32, Uuid), diesel::result::Error> {
                use crate::schema::uploads::dsl::*;

                let wid = Uuid::new_v4();
                let uid = diesel::insert_into(uploads)
                    .values(UploadInsert { web_id: wid })
                    .get_result::<Upload>(conn)?
                    .id;

                Ok((uid, wid))
            },
        )
        .await?;

    let (headers, cells, row_count) = parse_csv(file_stream, upload_id).await?;

    db.run(move |conn| {
        use crate::schema::upload_cells::dsl::*;

        conn.transaction::<_, diesel::result::Error, _>(|| {
            diesel::insert_into(upload_cells)
                .values(cells)
                .execute(conn)
        })
    })
    .await?;

    let header_suggestions = headers.iter().map(|h| HeaderOption::from_str(h)).collect();
    Ok(MoneyMsg::new(AddUploadResponse {
        upload_id: web_id,
        headers,
        header_suggestions,
        row_count,
    }))
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Money API", |rocket| async {
        rocket.mount("/api/upload", routes![add_upload])
    })
pub fn routes() -> Vec<Route> {
    routes![add_upload]
}
