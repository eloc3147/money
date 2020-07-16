use anyhow::Result;
use rocket::config::{Config, Environment};
use rocket_contrib::serve::StaticFiles;

fn rocket() -> Result<rocket::Rocket> {
    let config = Config::build(Environment::Staging).port(9234).finalize()?;

    Ok(rocket::custom(config).mount("/", StaticFiles::from("static")))
}

fn main() -> Result<()> {
    Err(rocket()?.launch().into())
}
