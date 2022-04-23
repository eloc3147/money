#[macro_use]
extern crate rocket;

use csv_async::{AsyncReader, Error};
use rocket::data::{Data, ToByteUnit};
use rocket::fs::FileServer;
use tokio_stream::StreamExt;

#[post("/", data = "<file>")]
async fn add_upload(file: Data<'_>) -> std::io::Result<()> {
    println!("New upload");
    let file_stream = file.open(10u8.mebibytes());
    let mut reader = AsyncReader::from_reader(file_stream);

    let headers = reader.headers().await?.iter();
    println!("Headers: {:?}", headers.collect::<Vec<&str>>().join(","));

    let rows = reader
        .records()
        .map(|row| -> Result<String, Error> { Ok(row?.iter().collect::<Vec<&str>>().join(",")) })
        .collect::<Result<Vec<String>, Error>>()
        .await?;

    println!("Rows: {:#?}", rows);

    Ok(())
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", FileServer::from("static"))
        .mount("/api/upload", routes![add_upload])
}
