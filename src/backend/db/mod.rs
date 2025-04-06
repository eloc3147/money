mod migrations;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::Local;
use log::{error, info};
use migrations::MIGRATIONS;
use rocket::fairing::AdHoc;
use rocket_db_pools::{
    sqlx::{self, Row, SqlitePool},
    Database,
};
use tokio::{self};

#[derive(Database)]
#[database("money_db")]
pub struct Db(SqlitePool);

pub fn setup_db(data_dir: PathBuf) -> AdHoc {
    AdHoc::try_on_ignite("Database Setup", move |rocket| async {
        let Some(db) = Db::fetch(&rocket) else {
            return Err(rocket);
        };

        if let Err(e) = setup_db_inner(db, data_dir).await {
            error!("{:?}", e);
            return Err(rocket);
        };

        Ok(rocket)
    })
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
        info!("Migrating database from version {}", version);

        backup_db(&db, &backup_dir, format!("backup_v{version}").as_str())
            .await
            .context("Failed to backup database")?;

        sqlx::raw_sql(&MIGRATIONS[version])
            .execute(&**db)
            .await
            .context(format!("Failed to migrate db from version {}", version))?;

        version += 1;
    }

    // Clear temp data
    sqlx::query(concat!(
        "DELETE FROM pending_upload_cells;",
        "DELETE FROM pending_uploads;"
    ))
    .execute(&**db)
    .await?;

    Ok(())
}

async fn backup_db(db: &Db, directory: &Path, prefix: &str) -> Result<()> {
    let backup_path = loop {
        let date_stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        let path = directory.join(format!("{}_{}.sqlite", prefix, date_stamp));

        if path.exists() {
            warn!(
                "Backup path \"{:?}\" already exists. Waiting for a new timestamp to retry",
                &path
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }

        break path;
    };

    sqlx::raw_sql(format!("VACUUM main INTO '{}'", backup_path.to_string_lossy()).as_str())
        .execute(&**db)
        .await?;

    Ok(())
}
