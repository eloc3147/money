//! The desktop application.
//!
//! [`MoneyApp`] owns the loaded data and one state struct per view. Views never touch the disk;
//! they return an [`Action`] which the app performs (saving the config and reloading).

pub mod chart;
pub mod error;
pub mod range;
pub mod settings;
pub mod style;
pub mod transactions;
pub mod uncategorized;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::NaiveDate;
use eframe::egui::{self, RichText, Ui};
use strum::{IntoStaticStr, VariantArray};

use crate::config::AppConfig;
use crate::error::ErrorReport;
use crate::loader::{LoadHandle, LoadOptions, Loaded, spawn_load};
use crate::model::Dataset;

/// Something a view asks the app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Write this config to disk and reload all data.
    SaveConfig(Box<AppConfig>),
}

/// The top level tabs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, VariantArray, IntoStaticStr, clap::ValueEnum,
)]
pub enum Tab {
    /// Transaction list.
    #[default]
    Transactions,
    /// Stacked bar chart.
    #[strum(serialize = "Spending by category")]
    Chart,
    /// Uncategorized transactions.
    Uncategorized,
    /// Config editor.
    Settings,
}

/// Where the app is in its load cycle.
enum LoadState {
    /// A load is running.
    Loading(LoadHandle),
    /// The last load succeeded.
    Ready,
    /// The last load failed.
    Failed(ErrorReport),
    /// There is no config file yet.
    NoConfig,
}

/// Requests a screenshot once data has loaded, then closes the window (used to test the real
/// application window).
struct ScreenshotRequest {
    /// Where to save the PNG.
    path: PathBuf,
    /// Frames rendered since loading finished.
    frames: u32,
    /// Set once the screenshot has been requested.
    requested: bool,
    /// Set by the callback once the PNG has been written.
    done: Arc<AtomicBool>,
}

/// The application.
pub struct MoneyApp {
    /// What to load.
    options: LoadOptions,
    /// Load status.
    state: LoadState,
    /// The config of the last successful load.
    config: AppConfig,
    /// The data of the last successful load.
    data: Dataset,
    /// The date relative ranges are based on.
    today: NaiveDate,
    /// Visible tab.
    pub tab: Tab,
    /// Transaction list state.
    pub transactions: transactions::TransactionsView,
    /// Chart state.
    pub chart: chart::ChartView,
    /// Uncategorized view state.
    pub uncategorized: uncategorized::UncategorizedView,
    /// Settings view state.
    pub settings: settings::SettingsView,
    /// Error display state.
    pub error_view: error::ErrorView,
    /// Optional screenshot-and-exit request.
    screenshot: Option<ScreenshotRequest>,
    /// Status message shown in the bottom bar.
    status: Option<String>,
}

impl MoneyApp {
    /// Create the app and start loading. `ctx` is used to wake the UI when loading finishes.
    #[must_use]
    pub fn new(options: LoadOptions, ctx: Option<egui::Context>, today: NaiveDate) -> Self {
        let mut app = Self::with_loaded(
            options,
            Loaded {
                config: AppConfig::default(),
                dataset: Dataset::default(),
            },
            today,
        );
        app.reload(ctx);
        app
    }

    /// Create the app with already loaded data (no background load).
    #[must_use]
    pub fn with_loaded(options: LoadOptions, loaded: Loaded, today: NaiveDate) -> Self {
        let mut settings = settings::SettingsView::default();
        settings.set_config(&loaded.config);
        Self {
            options,
            state: LoadState::Ready,
            config: loaded.config,
            data: loaded.dataset,
            today,
            tab: Tab::default(),
            transactions: transactions::TransactionsView::new(today),
            chart: chart::ChartView::new(today),
            uncategorized: uncategorized::UncategorizedView::default(),
            settings,
            error_view: error::ErrorView::default(),
            screenshot: None,
            status: None,
        }
    }

    /// Save a PNG of the window to `path` once loaded, then close.
    pub fn screenshot_and_exit(&mut self, path: PathBuf) {
        self.screenshot = Some(ScreenshotRequest {
            path,
            frames: 0,
            requested: false,
            done: Arc::new(AtomicBool::new(false)),
        });
    }

    /// The loaded data.
    #[must_use]
    pub fn data(&self) -> &Dataset {
        &self.data
    }

    /// The loaded config.
    #[must_use]
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Whether a load is in progress.
    #[must_use]
    pub fn is_loading(&self) -> bool {
        matches!(self.state, LoadState::Loading(_))
    }

    /// The error from the last load, if it failed.
    #[must_use]
    pub fn error(&self) -> Option<&ErrorReport> {
        match &self.state {
            LoadState::Failed(error) => Some(error),
            _ => None,
        }
    }

    /// Start a fresh load of config and transactions.
    pub fn reload(&mut self, ctx: Option<egui::Context>) {
        if !self.options.config_path.exists() {
            self.state = LoadState::NoConfig;
            self.tab = Tab::Settings;
            return;
        }
        let handle = spawn_load(self.options.clone(), move || {
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        self.state = LoadState::Loading(handle);
    }

    /// Check on a running load.
    fn poll_load(&mut self) {
        let LoadState::Loading(handle) = &self.state else {
            return;
        };
        match handle.try_take() {
            None => {}
            Some(Ok(loaded)) => {
                self.status = Some(format!(
                    "Loaded {} transactions ({} uncategorized)",
                    loaded.dataset.transactions.len(),
                    loaded.dataset.uncategorized.len()
                ));
                self.settings.set_config(&loaded.config);
                self.config = loaded.config;
                self.data = loaded.dataset;
                self.state = LoadState::Ready;
            }
            Some(Err(error)) => {
                // Keep the config editable even if the import failed.
                if let Ok(config) = AppConfig::load(&self.options.config_path) {
                    self.settings.set_config(&config);
                    self.config = config;
                }
                self.state = LoadState::Failed(error);
            }
        }
    }

    /// Perform an action requested by a view.
    pub fn perform(&mut self, action: Action, ctx: Option<egui::Context>) {
        match action {
            Action::SaveConfig(config) => match config.save(&self.options.config_path) {
                Ok(()) => {
                    self.settings.original = (*config).clone();
                    self.settings.draft = *config;
                    self.status = Some("Config saved, reloading…".into());
                    self.reload(ctx);
                }
                Err(error) => self.status = Some(format!("Failed to save config: {error:#}")),
            },
        }
    }

    /// Draw the whole UI. Also used directly by tests.
    pub fn ui(&mut self, ui: &mut Ui) {
        self.poll_load();
        let ctx = ui.ctx().clone();
        if self.is_loading() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("💰 Money").strong());
                ui.separator();
                for &tab in Tab::VARIANTS {
                    let mut label: String = Into::<&'static str>::into(tab).to_string();
                    if tab == Tab::Uncategorized && !self.data.uncategorized.is_empty() {
                        label = format!("{label} ({})", self.data.uncategorized.len());
                    }
                    ui.selectable_value(&mut self.tab, tab, label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(!self.is_loading(), egui::Button::new("⟳ Reload"))
                        .on_hover_text("Re-import all statement files")
                        .clicked()
                    {
                        self.reload(Some(ctx.clone()));
                    }
                    egui::widgets::global_theme_preference_buttons(ui);
                });
            });
        });

        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(status) = &self.status {
                    ui.weak(status);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(self.options.config_path.display().to_string());
                });
            });
        });

        let mut action = None;
        egui::CentralPanel::default_margins().show(ui, |ui| {
            if self.tab != Tab::Settings && !self.status_screen(ui) {
                return;
            }
            match self.tab {
                Tab::Transactions => self.transactions.ui(ui, &self.data, self.today),
                Tab::Chart => self.chart.ui(ui, &self.data, self.today),
                Tab::Uncategorized => action = self.uncategorized.ui(ui, &self.data, &self.config),
                Tab::Settings => action = self.settings.ui(ui, &self.options.config_path),
            }
        });
        if let Some(action) = action {
            self.perform(action, Some(ctx.clone()));
        }

        self.handle_screenshot(&ctx);
    }

    /// Show loading / error / welcome screens. Returns true when the data views can be shown.
    fn status_screen(&mut self, ui: &mut Ui) -> bool {
        match &self.state {
            LoadState::Ready => true,
            LoadState::Loading(handle) => {
                let status = &handle.status;
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.3);
                    ui.heading(status.stage().label());
                    let found = status.import.files_found();
                    let done = status.import.files_done();
                    #[allow(clippy::cast_precision_loss)]
                    let fraction = if found == 0 {
                        0.0
                    } else {
                        done as f32 / found as f32
                    };
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .desired_width(360.0)
                            .text(format!("{done} / {found} files")),
                    );
                    ui.label(format!(
                        "{} transactions read",
                        status.import.transactions()
                    ));
                    ui.spinner();
                });
                false
            }
            LoadState::Failed(error) => {
                let error = error.clone();
                ui.heading("Failed to load transactions");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("⟳ Retry").clicked() {
                        self.reload(Some(ui.ctx().clone()));
                    }
                    if ui.button("⚙ Open settings").clicked() {
                        self.tab = Tab::Settings;
                    }
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("error-scroll")
                    .show(ui, |ui| self.error_view.ui(ui, &error));
                false
            }
            LoadState::NoConfig => {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.3);
                    ui.heading("Welcome to Money");
                    ui.label(format!(
                        "No config file found at {}.",
                        self.options.config_path.display()
                    ));
                    if ui.button("⚙ Create one in Settings").clicked() {
                        self.tab = Tab::Settings;
                    }
                });
                false
            }
        }
    }

    /// Drive a pending screenshot request.
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let loading = self.is_loading();
        let Some(request) = &mut self.screenshot else {
            return;
        };
        if request.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if loading {
            return;
        }
        request.frames += 1;
        ctx.request_repaint();
        // Give layouts (tables, plots) a few frames to settle.
        if request.frames > 5 && !request.requested {
            request.requested = true;
            let path = request.path.clone();
            let done = Arc::clone(&request.done);
            let repaint = ctx.clone();
            ctx.request_screenshot(move |image| {
                let [w, h] = image.size;
                let bytes = image.as_raw().to_vec();
                if let Some(buffer) = image::RgbaImage::from_raw(
                    u32::try_from(w).unwrap_or(0),
                    u32::try_from(h).unwrap_or(0),
                    bytes,
                ) && let Err(error) = buffer.save(&path)
                {
                    eprintln!("Failed to save screenshot: {error}");
                }
                done.store(true, Ordering::SeqCst);
                repaint.request_repaint();
            });
        }
    }
}

impl eframe::App for MoneyApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        MoneyApp::ui(self, ui);
    }
}

/// Open the application window.
///
/// # Errors
/// Fails if the window or graphics context cannot be created.
pub fn run(options: LoadOptions, screenshot: Option<PathBuf>, tab: Tab) -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Money")
            .with_app_id("money")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Money",
        native_options,
        Box::new(move |cc| {
            let today = chrono::Local::now().date_naive();
            let mut app = MoneyApp::new(options, Some(cc.egui_ctx.clone()), today);
            if app.tab != Tab::Settings {
                app.tab = tab;
            }
            if let Some(path) = screenshot {
                app.screenshot_and_exit(path);
            }
            Ok(Box::new(app))
        }),
    )
}
