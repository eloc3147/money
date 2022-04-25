use csv_async::{self, AsyncReader};
use rocket::data::{Data, DataStream, ToByteUnit};
use rocket::fairing::AdHoc;
use rocket::futures::StreamExt;

use diesel::prelude::*;
use diesel::{Connection, RunQueryDsl};

use crate::error::Result;
use crate::models::{Upload, UploadCell};
use crate::Db;

async fn parse_csv(stream: DataStream<'_>, upload_id: i32) -> Result<Vec<UploadCell>> {
    let mut reader = AsyncReader::from_reader(stream);
    let mut cells = Vec::new();

    for (column_num, cell) in reader.headers().await?.iter().enumerate() {
        cells.push(UploadCell {
            id: None,
            upload_id,
            header: true,
            row_num: 0,
            column_num: column_num as i64,
            contents: cell.to_string(),
        });
    }

    let mut records = reader.records().enumerate();

    while let Some((row_num, row)) = records.next().await {
        for (column_num, cell) in row?.iter().enumerate() {
            cells.push(UploadCell {
                id: None,
                upload_id,
                header: false,
                row_num: row_num as i64,
                column_num: column_num as i64,
                contents: cell.to_string(),
            });
        }
    }

    Ok(cells)
}

#[post("/", data = "<file>")]
async fn add_upload(db: Db, file: Data<'_>) -> Result<()> {
    let file_stream = file.open(10u8.mebibytes());

    let upload_id = db
        .run(move |conn| {
            use crate::schema::uploads::dsl::*;

            conn.transaction::<_, diesel::result::Error, _>(|| {
                diesel::insert_into(uploads)
                    .default_values()
                    .execute(conn)?;

                Ok(uploads.order(id.desc()).first::<Upload>(conn)?.id)
            })
        })
        .await?;

    let cells = parse_csv(file_stream, upload_id).await?;

    db.run(move |conn| {
        use crate::schema::upload_cells::dsl::*;

        conn.transaction::<_, diesel::result::Error, _>(|| {
            diesel::insert_into(upload_cells)
                .values(cells)
                .execute(conn)
        })
    })
    .await?;

    Ok(())
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Money API", |rocket| async {
        rocket.mount("/api/upload", routes![add_upload])
    })
}
