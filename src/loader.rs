//! Loads the config, imports every transaction file into SQLite and reads back a [`Dataset`].
//!
//! [`spawn_load`] runs the whole pipeline on a background thread so the GUI stays responsive.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};

use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};

use crate::config::{AppConfig, BudgetConfig};
use crate::db::{Db, DbLocation};
use crate::importer::categorizer::Categorizer;
use crate::importer::{ImportProgress, import_files};
use crate::model::Dataset;

/// What to load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadOptions {
    /// The config file.
    pub config_path: PathBuf,
    /// Database file to use when the config does not set one. `None` uses an in-memory database.
    pub default_db_path: Option<PathBuf>,
}

/// The pipeline step currently running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stage {
    /// Reading the config file.
    #[default]
    LoadingConfig,
    /// Building the categorizer.
    BuildingRules,
    /// Creating the database and storing budgets.
    OpeningDatabase,
    /// Importing transaction files.
    Importing,
    /// Reading the imported data back.
    ReadingData,
}

impl Stage {
    /// Human readable description.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::LoadingConfig => "Loading config…",
            Self::BuildingRules => "Building rules…",
            Self::OpeningDatabase => "Opening database…",
            Self::Importing => "Importing transaction files…",
            Self::ReadingData => "Reading transactions…",
        }
    }
}

/// Progress shared between the loader and the GUI.
#[derive(Debug, Default)]
pub struct LoadStatus {
    /// Current step.
    stage: Mutex<Stage>,
    /// File and transaction counters.
    pub import: ImportProgress,
}

impl LoadStatus {
    /// The step currently running.
    #[must_use]
    pub fn stage(&self) -> Stage {
        *self.stage.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Move to a new step.
    fn set_stage(&self, stage: Stage) {
        *self.stage.lock().unwrap_or_else(PoisonError::into_inner) = stage;
    }
}

/// The result of a successful load.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The config that was used.
    pub config: AppConfig,
    /// The imported data.
    pub dataset: Dataset,
}

/// Store budgets, each ending when the next one starts.
async fn store_budgets(db: &Db, configs: &[BudgetConfig]) -> Result<()> {
    let mut handle = db.start_transaction().await?;
    for (idx, budget) in configs.iter().enumerate() {
        let end = configs.get(idx + 1).map(|next| next.start_date);
        for rule in &budget.rules {
            handle
                .add_budget(
                    &budget.name,
                    budget.start_date,
                    end,
                    &rule.category,
                    rule.limit,
                )
                .await?;
        }
    }
    handle.commit().await
}

/// Run the full load pipeline with an already parsed config.
///
/// # Errors
/// Fails if the rules are invalid, the database cannot be opened or any file fails to import.
pub async fn load_with_config(
    config: AppConfig,
    default_db_path: Option<PathBuf>,
    status: &LoadStatus,
) -> Result<Loaded> {
    status.set_stage(Stage::BuildingRules);
    let categorizer = Categorizer::build(&config.transaction_type, &config.rule)
        .wrap_err("Failed to load transaction rules")?;

    status.set_stage(Stage::OpeningDatabase);
    let db_path = config.database.path.clone().or(default_db_path);
    let db = match &db_path {
        Some(path) => Db::open(DbLocation::File(path)).await,
        None => Db::open(DbLocation::Memory).await,
    }
    .wrap_err("Failed to setup DB")?;
    store_budgets(&db, &config.budget).await?;

    status.set_stage(Stage::Importing);
    import_files(&db, &categorizer, &config.account, &status.import).await?;

    status.set_stage(Stage::ReadingData);
    let dataset = db.dataset().await?;
    Ok(Loaded { config, dataset })
}

/// Run the full load pipeline.
///
/// # Errors
/// Fails if the config cannot be read or [`load_with_config`] fails.
pub async fn load(options: &LoadOptions, status: &LoadStatus) -> Result<Loaded> {
    status.set_stage(Stage::LoadingConfig);
    let path = options.config_path.clone();
    let config = tokio::task::spawn_blocking(move || AppConfig::load(&path))
        .await?
        .wrap_err("Failed to load config")?;
    load_with_config(config, options.default_db_path.clone(), status).await
}

/// A load running on a background thread.
pub struct LoadHandle {
    /// Live progress.
    pub status: Arc<LoadStatus>,
    /// Receives the result once the load finishes.
    result: Receiver<Result<Loaded>>,
}

impl LoadHandle {
    /// Return the result if the load has finished.
    pub fn try_take(&self) -> Option<Result<Loaded>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(eyre!("Loader thread exited unexpectedly")))
            }
        }
    }

    /// Block until the load finishes.
    ///
    /// # Errors
    /// Returns the load error, if any.
    pub fn wait(self) -> Result<Loaded> {
        self.result
            .recv()
            .map_err(|_| eyre!("Loader thread exited unexpectedly"))?
    }
}

/// Start loading on a background thread. `on_done` is called after the result is sent (the
/// GUI uses it to request a repaint).
#[must_use]
pub fn spawn_load(options: LoadOptions, on_done: impl FnOnce() + Send + 'static) -> LoadHandle {
    let status = Arc::new(LoadStatus::default());
    let (tx, rx) = std::sync::mpsc::channel();
    let thread_status = Arc::clone(&status);
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .wrap_err("Failed to start async runtime")
            .and_then(|rt| rt.block_on(load(&options, &thread_status)));
        // The receiver may have been dropped if the GUI started another load; that's fine.
        let _ = tx.send(result);
        on_done();
    });
    LoadHandle { status, result: rx }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;

    #[test]
    fn spawn_load_imports_mock_data() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = mock::write_mock_data(dir.path(), mock::MockOptions::default()).unwrap();
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done2 = Arc::clone(&done);
        let handle = spawn_load(
            LoadOptions {
                config_path,
                default_db_path: Some(dir.path().join("money.sqlite")),
            },
            move || done2.store(true, std::sync::atomic::Ordering::SeqCst),
        );
        let status = Arc::clone(&handle.status);
        let loaded = handle.wait().unwrap();
        assert!(!loaded.dataset.transactions.is_empty());
        assert!(!loaded.dataset.uncategorized.is_empty());
        assert_eq!(status.stage(), Stage::ReadingData);
        assert!(status.import.files_done() >= 3);
        assert_eq!(status.import.files_done(), status.import.files_found());
        assert!(status.import.transactions() > 0);
        assert!(dir.path().join("money.sqlite").exists());
    }

    #[test]
    fn load_reports_missing_config() {
        let dir = tempfile::tempdir().unwrap();
        let handle = spawn_load(
            LoadOptions {
                config_path: dir.path().join("missing.toml"),
                default_db_path: None,
            },
            || {},
        );
        let err = loop {
            if let Some(result) = handle.try_take() {
                break result.unwrap_err();
            }
            std::thread::yield_now();
        };
        assert!(format!("{err:#}").contains("config"), "{err:#}");
    }

    #[tokio::test]
    async fn load_reports_missing_account_dir() {
        let mut config = AppConfig::default();
        config.account.push(crate::config::AccountConfig {
            name: "A".into(),
            source_path: "/definitely/not/here".into(),
        });
        let err = load_with_config(config, None, &LoadStatus::default())
            .await
            .unwrap_err();
        assert!(
            format!("{err:#}").contains("/definitely/not/here"),
            "{err:#}"
        );
    }

    #[test]
    fn stage_labels() {
        for stage in [
            Stage::LoadingConfig,
            Stage::BuildingRules,
            Stage::OpeningDatabase,
            Stage::Importing,
            Stage::ReadingData,
        ] {
            assert!(stage.label().ends_with('…'));
        }
    }
}
