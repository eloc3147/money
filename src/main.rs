#[macro_use]
extern crate rocket;

mod api;
mod backend;
mod error;

use error::Result;
use rocket::{fs::FileServer, log::LogLevel, Config};
use yansi::Paint;

async fn run() -> Result<()> {
    let data_dir = dirs::data_dir()
        .expect("OS User data directory missing")
        .join("money_app");

    println!(
        "{} {}",
        Paint::blue("Loading data from"),
        &data_dir.to_string_lossy()
    );

    let data = backend::Backend::load(data_dir).await?;

    let mut config = Config::from(Config::figment());
    config.log_level = LogLevel::Debug;

    println!("{}", Paint::blue("Launching web server."));
    let _ = rocket::custom(config)
        .attach(api::stage())
        .manage(data)
        .mount("/", FileServer::from("static"))
        .launch()
        .await;

    Ok(())
}

#[rocket::main]
async fn main() {
    println!(
        "\n{} {}\n",
        Paint::green("MONEY").bold(),
        env!("CARGO_PKG_VERSION"),
    );
    match run().await {
        Ok(_) => println!("\n{}", Paint::blue("Money app exiting")),
        Err(e) => {
            println!("\n{}", Paint::red("Money app crashed").bold());
            println!("{}", Paint::yellow(e).bold());
        }
    }
}
