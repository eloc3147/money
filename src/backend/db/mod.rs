mod migrations;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use log::{error, info};
use rocket::fairing::AdHoc;
use rocket::{fairing, Build, Rocket};
use rocket_db_pools::sqlx::{self, Row, SqlitePool};
use rocket_db_pools::Database;
use tokio;

use migrations::MIGRATIONS;

#[derive(Database)]
#[database("money_db")]
pub struct Db(SqlitePool);

pub fn build_fairing(data_dir: PathBuf) -> AdHoc {
    AdHoc::try_on_ignite("Database Setup", move |rocket| setup_db(rocket, data_dir))
}

async fn setup_db(rocket: Rocket<Build>, data_dir: PathBuf) -> fairing::Result {
    let Some(db) = Db::fetch(&rocket) else {
        return Err(rocket);
    };

    if let Err(e) = setup_db_inner(db, data_dir).await {
        error!("{:?}", e);
        return Err(rocket);
    };

    Ok(rocket)
}

async fn setup_db_inner(db: &Db, data_dir: PathBuf) -> Result<()> {
    let mut version = sqlx::query("SELECT version FROM metadata;")
        .fetch_one(&**db)
        .await
        .and_then(|r| r.try_get::<u32, usize>(0).map(|v| v as usize))
        .unwrap_or(0);

    info!("Current database version: {}", version);

    let backup_dir = data_dir.join("backups");
    tokio::fs::create_dir_all(&backup_dir)
        .await
        .context("Failed to crate backup directory")?;

    while version < MIGRATIONS.len() {
        info!("Migrating database to version {}", version + 1);

        backup_db(&db, &backup_dir.join(format!("backup_v{version}.sqlite")))
            .await
            .context("Failed to backup database")?;

        sqlx::raw_sql(&MIGRATIONS[version])
            .execute(&**db)
            .await
            .context(format!("Failed to apply migration v{}", version))?;

        version += 1;
    }

    Ok(())
}

async fn backup_db(db: &Db, path: &Path) -> Result<()> {
    sqlx::raw_sql(format!("VACUUM main INTO '{}'", path.to_string_lossy()).as_str())
        .execute(&**db)
        .await?;

    Ok(())
}
