#[macro_use]
extern crate rocket;

mod api;
mod components;
mod data_store;
mod error;

use std::path::PathBuf;

use rocket::fs::FileServer;

#[launch]
fn rocket() -> _ {
    let data = data_store::DataStore::load(&PathBuf::new());

    rocket::build()
        .attach(api::stage())
        .manage(data)
        .mount("/", FileServer::from("static"))
}
