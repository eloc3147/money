#[macro_use]
extern crate rocket;

use common::SubmitDataRequest;
use rocket::fs::FileServer;
use rocket::serde::json::Json;

#[post("/add_transactions", format = "json", data = "<request>")]
fn add_transactions(request: Json<SubmitDataRequest>) {
    println!("Got add_transactions request: {:#?}", request);
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", FileServer::from("static"))
        .mount("/api", routes![add_transactions])
}
