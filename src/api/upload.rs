use std::fmt;

use csv_async::{self, AsyncReader};
use diesel::{prelude::*, Connection, RunQueryDsl};
use rocket::{
    data::{Data, DataStream, ToByteUnit},
    fairing::AdHoc,
    futures::StreamExt,
    serde::{
        de::{self, Visitor},
        json::Json,
        Deserialize, Deserializer, Serialize, Serializer,
    },
};
use uuid::Uuid;

use crate::error::Result;
use crate::models::{Upload, UploadCell};
use crate::Db;

async fn parse_csv(
    stream: DataStream<'_>,
    upload_id: i32,
) -> Result<(Vec<String>, Vec<UploadCell>)> {
    let mut reader = AsyncReader::from_reader(stream);
    let mut headers = Vec::new();
    let mut cells = Vec::new();

    for (column_num, cell) in reader.headers().await?.iter().enumerate() {
        headers.push(cell.to_string());
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

    Ok((headers, cells))
}

#[derive(Clone, PartialEq, Copy, Debug)]
enum HeaderOption {
    Unused,
    Date,
    Name,
    Description,
    Amount,
}

impl HeaderOption {
    fn to_str(&self) -> &'static str {
        match self {
            HeaderOption::Unused => "-",
            HeaderOption::Date => "Date",
            HeaderOption::Name => "Name",
            HeaderOption::Description => "Description",
            HeaderOption::Amount => "Amount",
        }
    }

    fn from_str(string: &str) -> HeaderOption {
        match string.trim().to_lowercase().as_ref() {
            "date" => HeaderOption::Date,
            "name" => HeaderOption::Name,
            "memo" | "description" => HeaderOption::Description,
            "amount" => HeaderOption::Amount,
            _ => HeaderOption::Unused,
        }
    }
}

impl Serialize for HeaderOption {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_str())
    }
}

struct HeaderOptionVisitor;

impl<'de> Visitor<'de> for HeaderOptionVisitor {
    type Value = HeaderOption;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(HeaderOption::from_str(value))
    }
}

impl<'de> Deserialize<'de> for HeaderOption {
    fn deserialize<D>(deserializer: D) -> std::result::Result<HeaderOption, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(HeaderOptionVisitor)
    }
}

#[derive(Clone, PartialEq, Serialize)]
struct AddUploadResponse {
    upload_id: Uuid,
    headers: Vec<String>,
    header_suggestions: Vec<HeaderOption>,
}

#[post("/", data = "<file>")]
async fn add_upload(db: Db, file: Data<'_>) -> Result<Json<AddUploadResponse>> {
    let file_stream = file.open(10u8.mebibytes());

    let (upload_id, web_id) = db
        .run(move |conn| {
            use crate::schema::uploads::dsl::*;

            let wid = Uuid::new_v4();
            conn.transaction::<_, diesel::result::Error, _>(|| {
                diesel::insert_into(uploads)
                    .values(web_id.eq(wid))
                    .execute(conn)?;

                let uid = uploads.order(id.desc()).first::<Upload>(conn)?.id;
                Ok((uid, wid))
            })
        })
        .await?;

    let (headers, cells) = parse_csv(file_stream, upload_id).await?;

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
    Ok(Json(AddUploadResponse {
        upload_id: web_id,
        headers,
        header_suggestions,
    }))
}

pub fn stage() -> AdHoc {
    AdHoc::on_ignite("Money API", |rocket| async {
        rocket.mount("/api/upload", routes![add_upload])
    })
}
