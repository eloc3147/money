#[macro_use]
extern crate rocket;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod api;
mod components;
mod error;
mod models;
mod schema;

use rocket::{fairing::AdHoc, fs::FileServer, Build, Rocket};
use rocket_sync_db_pools::database;

#[database("money")]
pub struct Db(diesel::PgConnection);

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    embed_migrations!("migrations");

    let conn = Db::get_one(&rocket).await.expect("database connection");
    conn.run(|c| embedded_migrations::run(c))
        .await
        .expect("diesel migrations");

    rocket
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(Db::fairing())
        .attach(api::stage())
        .attach(AdHoc::on_ignite("Database Migrations", run_migrations))
        .mount("/", FileServer::from("static"))
}
