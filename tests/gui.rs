//! End-to-end GUI tests: drive the real [`MoneyApp`] headlessly with mock data.

use std::path::Path;

use chrono::NaiveDate;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use money::gui::{MoneyApp, Tab};
use money::loader::{LoadOptions, LoadStatus, load};
use money::mock::{MockOptions, write_mock_data};

/// Fixed "today" so relative ranges are deterministic.
fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2025, 6, 30).unwrap()
}

/// Generate and import mock data under `dir`.
fn mock_app(dir: &Path) -> MoneyApp {
    let config_path = write_mock_data(
        dir,
        MockOptions {
            end: today(),
            months: 24,
            seed: 7,
        },
    )
    .unwrap();
    let options = LoadOptions {
        config_path,
        default_db_path: None,
    };
    let loaded = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(load(&options, &LoadStatus::default()))
        .unwrap();
    MoneyApp::with_loaded(options, loaded, today())
}

/// A harness around the app at a desktop-like size.
fn harness(app: MoneyApp) -> Harness<'static, MoneyApp> {
    let mut harness = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 820.0))
        .build_ui_state(|ui, app: &mut MoneyApp| app.ui(ui), app);
    harness.run_ok();
    harness
}

/// Serializes renders: concurrent software (lavapipe) rendering from parallel tests has crashed.
static RENDER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Save a rendering of the harness for manual inspection when `MONEY_SNAPSHOT_DIR` is set.
fn maybe_render(harness: &mut Harness<'_, MoneyApp>, name: &str) {
    if let Ok(dir) = std::env::var("MONEY_SNAPSHOT_DIR") {
        let _guard = RENDER_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let image = harness.render().expect("render");
        image
            .save(Path::new(&dir).join(format!("{name}.png")))
            .unwrap();
    }
}

#[test]
fn renders_every_tab() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = harness(mock_app(dir.path()));
    for (tab, name) in [
        (Tab::Transactions, "transactions"),
        (Tab::Chart, "chart"),
        (Tab::Uncategorized, "uncategorized"),
        (Tab::Settings, "settings"),
    ] {
        harness.state_mut().tab = tab;
        harness.run_ok();
        maybe_render(&mut harness, name);
    }
}

/// Step the harness until any background load finishes.
fn wait_for_load(harness: &mut Harness<'_, MoneyApp>) {
    for _ in 0..600 {
        harness.step();
        if !harness.state().is_loading() {
            harness.run_ok();
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("load did not finish");
}

#[test]
fn tabs_switch_by_clicking() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = harness(mock_app(dir.path()));
    harness.get_by_label("Spending by category").click();
    harness.run_ok();
    assert_eq!(harness.state().tab, Tab::Chart);
    harness.get_by_label("Settings").click();
    harness.run_ok();
    assert_eq!(harness.state().tab, Tab::Settings);
    harness.get_by_label_contains("Uncategorized (").click();
    harness.run_ok();
    assert_eq!(harness.state().tab, Tab::Uncategorized);
}

#[test]
fn transactions_toggle_income_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = harness(mock_app(dir.path()));
    harness.get_by_label("Income").click();
    harness.run_ok();
    assert!(harness.state().transactions.filter.side.is_income());
    // Income rows are on screen; an expense merchant is not.
    assert!(
        harness
            .query_all_by_label("PAYROLL ACME CORP")
            .next()
            .is_some()
    );
    assert!(harness.query_by_label("POS GROCERY MART").is_none());

    harness.get_by_label("Expenses").click();
    harness.run_ok();
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .focus();
    harness.run_ok();
    harness
        .get_by_role(eframe::egui::accesskit::Role::TextInput)
        .type_text("streamflix");
    harness.run_ok();
    assert_eq!(harness.state().transactions.filter.search, "streamflix");
    assert!(harness.query_by_label("POS GROCERY MART").is_none());
    assert!(
        !harness
            .query_all_by_label("STREAMFLIX")
            .collect::<Vec<_>>()
            .is_empty()
    );
}

#[test]
fn chart_controls_change_grouping() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = mock_app(dir.path());
    app.tab = Tab::Chart;
    let mut harness = harness(app);
    let combo = |harness: &Harness<'_, MoneyApp>, label: &str| {
        harness
            .get_all_by_role_and_label(eframe::egui::accesskit::Role::ComboBox, label)
            .next()
            .unwrap()
            .click();
    };
    combo(&harness, "Group by:");
    harness.run_ok();
    harness.get_by_label("Quarter").click();
    harness.run_ok();
    assert_eq!(
        harness.state().chart.period,
        money::analysis::Period::Quarter
    );
    assert!(
        harness.query_by_label("Per quarter").is_some(),
        "breakdown uses quarters"
    );

    combo(&harness, "Averaging:");
    harness.run_ok();
    harness.get_by_label("Moving average").click();
    harness.run_ok();
    assert_eq!(
        harness.state().chart.averaging,
        money::analysis::Averaging::Moving
    );
    maybe_render(&mut harness, "chart-quarter-moving");
}

#[test]
fn adding_rule_categorizes_and_reloads() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = mock_app(dir.path());
    app.tab = Tab::Uncategorized;
    let before = app.data().uncategorized.len();
    let bookstore = app
        .data()
        .uncategorized
        .iter()
        .filter(|u| u.message == "BOOKSTORE")
        .count();
    assert!(bookstore > 0);
    let mut harness = harness(app);

    harness.get_by_label("BOOKSTORE").click();
    harness.run_ok();
    maybe_render(&mut harness, "uncategorized-form");
    let input = eframe::egui::accesskit::Role::TextInput;
    harness.get_by_role_and_label(input, "Category").focus();
    harness.run_ok();
    harness
        .get_by_role_and_label(input, "Category")
        .type_text("Shopping.Books");
    harness.run_ok();
    harness.get_by_label("Add rule and reload").click();
    harness.run_ok();
    wait_for_load(&mut harness);

    let app = harness.state();
    assert_eq!(app.error(), None);
    assert_eq!(app.data().uncategorized.len(), before - bookstore);
    assert!(
        app.data()
            .transactions
            .iter()
            .any(|t| t.category == "Shopping.Books")
    );
    assert!(
        app.config()
            .categories()
            .contains(&"Shopping.Books".to_string())
    );
    assert!(
        dir.path().join("config.toml.bak").exists(),
        "previous config backed up"
    );
}

#[test]
fn settings_edit_and_save() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = mock_app(dir.path());
    app.tab = Tab::Settings;
    let mut harness = harness(app);
    assert!(!harness.state().settings.is_dirty());
    for section in [
        "Transaction types",
        "Rules",
        "Budgets",
        "Advanced",
        "Accounts",
    ] {
        harness.get_by_label(section).click();
        harness.run_ok();
        maybe_render(
            &mut harness,
            &format!("settings-{}", section.to_lowercase().replace(' ', "-")),
        );
    }
    harness.get_by_label("➕ Add account").click();
    harness.run_ok();
    assert!(harness.state().settings.is_dirty());
    harness.get_by_label("↺ Revert").click();
    harness.run_ok();
    assert!(!harness.state().settings.is_dirty());

    harness.get_by_label("Budgets").click();
    harness.run_ok();
    harness.get_by_label("➕ Add budget").click();
    harness.run_ok();
    harness.get_by_label("💾 Save and reload").click();
    harness.run_ok();
    wait_for_load(&mut harness);
    let saved = money::config::AppConfig::load(&dir.path().join("config.toml")).unwrap();
    assert_eq!(saved.budget.len(), 1);
    assert!(!harness.state().settings.is_dirty());
}

#[test]
fn missing_config_opens_settings() {
    let dir = tempfile::tempdir().unwrap();
    let app = MoneyApp::new(
        LoadOptions {
            config_path: dir.path().join("config.toml"),
            default_db_path: None,
        },
        None,
        today(),
    );
    let mut harness = harness(app);
    assert_eq!(harness.state().tab, Tab::Settings);
    harness.get_by_label("Transactions").click();
    harness.run_ok();
    assert!(
        harness
            .query_by_label_contains("No config file found")
            .is_some()
    );
}

#[test]
fn import_errors_are_shown() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    std::fs::write(
        &config_path,
        "[[account]]\nname = \"A\"\nsource_path = \"/definitely/missing\"\n",
    )
    .unwrap();
    let app = MoneyApp::new(
        LoadOptions {
            config_path,
            default_db_path: None,
        },
        None,
        today(),
    );
    let mut harness = harness(app);
    wait_for_load(&mut harness);
    let report = harness.state().error().unwrap().clone();
    assert!(
        report.to_string().contains("/definitely/missing"),
        "{report}"
    );
    assert!(
        !report.to_string().contains('\u{1b}'),
        "no ANSI color codes"
    );
    assert!(
        harness
            .query_by_label("Failed to load transactions")
            .is_some()
    );
    // The message, each cause and the stack trace are shown as separate widgets.
    assert!(harness.query_by_label(&report.message).is_some());
    for cause in &report.causes {
        assert!(
            harness.query_by_label(cause).is_some(),
            "cause {cause:?} shown"
        );
    }
    assert!(harness.query_by_label_contains("Stack trace (").is_some());
    harness.get_by_label_contains("Show all ").click();
    harness.run_ok();
    assert!(harness.state().error_view.show_all_frames);
    maybe_render(&mut harness, "load-error");
    // The config is still editable after a failed import.
    assert_eq!(harness.state().config().account.len(), 1);
    harness.get_by_label("⚙ Open settings").click();
    harness.run_ok();
    assert_eq!(harness.state().tab, Tab::Settings);
}
