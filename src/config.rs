use std::path::Path;

use rocket::figment::util::map;
use rocket::figment::Figment;
use rocket::{log::LogLevel, Config};

pub fn fetch(data_dir: &Path) -> Figment {
    Config::figment()
        .merge(("log_level", LogLevel::Normal))
        .merge((
            "databases",
            map!["money_db" => map!["url" => data_dir.join("money_db.sqlite")]],
        ))
}
