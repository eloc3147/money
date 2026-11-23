//! End-to-end GUI tests: drive the real [`MoneyApp`] headlessly with mock data.

use std::path::Path;

use chrono::NaiveDate;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use money::gui::{MoneyApp, Options, Tab};
use money::loader::{LoadStatus, load};
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
    let options = Options {
        config_path: config_path.clone(),
        #[cfg(feature = "dev-mode")]
        screenshot: None,
    };
    let loaded = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(load(config_path, &LoadStatus::default()))
        .unwrap();
    MoneyApp::with_loaded(options, loaded, today())
}

/// Like [`mock_app`], but every third transaction belongs to an account with a very long name.
fn long_account_app(dir: &Path) -> MoneyApp {
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
        config_path: config_path.clone(),
        default_db_path: None,
    };
    let mut loaded = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(load(config_path, &LoadStatus::default()))
        .unwrap();
    for t in loaded
        .dataset
        .transactions
        .iter_mut()
        .filter(|t| t.id % 3 == 0)
    {
        t.account = "Capital One World Elite Mastercard Rewards".into();
    }
    MoneyApp::with_loaded(options, loaded, today())
}

/// Right edges of every signed amount label on screen.
fn amount_right_edges(harness: &Harness<'_, MoneyApp>) -> Vec<f32> {
    harness
        .get_all_by_role(eframe::egui::accesskit::Role::Label)
        .filter(|n| {
            // Label text is exposed as the node's value.
            n.accesskit_node()
                .value()
                .is_some_and(|l| l.starts_with('\u{2212}') || l.starts_with('+'))
        })
        .map(|n| n.rect().right())
        .collect()
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
        for (theme, suffix) in [
            (eframe::egui::ThemePreference::Light, "light"),
            (eframe::egui::ThemePreference::Dark, "dark"),
        ] {
            harness.ctx.set_theme(theme);
            harness.run_ok();
            let image = harness.render().expect("render");
            image
                .save(Path::new(&dir).join(format!("{name}-{suffix}.png")))
                .unwrap();
        }
    }
}

/// Click the button named `label` (page titles can share a navigation item's name).
fn click_button(harness: &Harness<'_, MoneyApp>, label: &str) {
    harness
        .get_by_role_and_label(eframe::egui::accesskit::Role::Button, label)
        .click();
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
    click_button(&harness, "Spending by category");
    harness.run_ok();
    assert_eq!(harness.state().tab, Tab::Chart);
    click_button(&harness, "Settings");
    harness.run_ok();
    assert_eq!(harness.state().tab, Tab::Settings);
    click_button(&harness, "Uncategorized");
    harness.run_ok();
    assert_eq!(harness.state().tab, Tab::Uncategorized);
}

#[test]
fn table_columns_stay_aligned_with_long_content() {
    let dir = tempfile::tempdir().unwrap();
    let mut harness = harness(long_account_app(dir.path()));
    for _ in 0..3 {
        let edges = amount_right_edges(&harness);
        assert!(edges.len() > 5, "amounts on screen");
        let (min, max) = edges
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(max - min < 0.5, "amounts share one right edge: {edges:?}");
        // Card content ends at window width - page margin (32) - card padding (20).
        assert!(
            max <= 1280.0 - 32.0 - 20.0,
            "table stays inside its card: {max}"
        );
        harness
            .get_all_by_role(eframe::egui::accesskit::Role::Label)
            .find(|n| {
                n.accesskit_node()
                    .value()
                    .is_some_and(|l| l.ends_with(", 2025"))
            })
            .unwrap()
            .scroll_down();
        harness.run_ok();
    }
    maybe_render(&mut harness, "long-accounts");
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
    click_button(&harness, "Quarter");
    harness.run_ok();
    assert_eq!(
        harness.state().chart.period,
        money::analysis::Period::Quarter
    );
    assert!(
        harness.query_by_label("Per quarter").is_some(),
        "breakdown uses quarters"
    );

    combo(&harness, "Averaging");
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
fn chart_height_follows_drag() {
    use eframe::egui::{Event, Modifiers, PointerButton, pos2};

    let dir = tempfile::tempdir().unwrap();
    let mut app = mock_app(dir.path());
    app.tab = Tab::Chart;
    let mut harness = harness(app);
    let before = harness.state().chart.plot_height;
    let start = harness
        .get_by_role_and_label(eframe::egui::accesskit::Role::Splitter, "Resize chart")
        .rect()
        .center();
    let end = pos2(start.x, start.y + 120.0);
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    harness.event(Event::PointerMoved(start));
    harness.event(press(start, true));
    harness.step();
    for i in 1..=6 {
        #[allow(clippy::cast_precision_loss)]
        harness.event(Event::PointerMoved(pos2(
            start.x,
            start.y + 20.0 * i as f32,
        )));
        harness.step();
    }
    harness.event(press(end, false));
    harness.run_ok();
    let after = harness.state().chart.plot_height;
    assert!(
        (after - before - 120.0).abs() < 1.0,
        "plot grew with the drag: {before} -> {after}"
    );
    maybe_render(&mut harness, "chart-tall");
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
        "Display",
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
    click_button(&harness, "Add account");
    harness.run_ok();
    assert!(harness.state().settings.is_dirty());
    click_button(&harness, "Revert");
    harness.run_ok();
    assert!(!harness.state().settings.is_dirty());

    harness.get_by_label("Budgets").click();
    harness.run_ok();
    click_button(&harness, "Add budget");
    harness.run_ok();
    click_button(&harness, "Save and reload");
    harness.run_ok();
    wait_for_load(&mut harness);
    let saved = money::config::AppConfig::load(&dir.path().join("config.toml")).unwrap();
    assert_eq!(saved.budget.len(), 1);
    assert!(!harness.state().settings.is_dirty());
}

#[test]
fn palette_setting_saves_and_recolors() {
    use money::config::ChartPalette;
    use strum::VariantArray;

    let dir = tempfile::tempdir().unwrap();
    let mut harness = harness(mock_app(dir.path()));
    assert_eq!(
        harness.state().config().display.chart_palette,
        ChartPalette::ThemeGlasbey
    );
    // Pick every palette in turn (default last, so each pick is a change to save).
    let order = ChartPalette::VARIANTS
        .iter()
        .copied()
        .cycle()
        .skip(1)
        .take(ChartPalette::VARIANTS.len());
    for palette in order {
        click_button(&harness, "Settings");
        harness.run_ok();
        click_button(&harness, "Display");
        harness.run_ok();
        harness
            .get_by_role_and_label(eframe::egui::accesskit::Role::RadioButton, palette.label())
            .click();
        harness.run_ok();
        click_button(&harness, "Save and reload");
        harness.run_ok();
        wait_for_load(&mut harness);
        assert_eq!(harness.state().config().display.chart_palette, palette);
        let saved = money::config::AppConfig::load(&dir.path().join("config.toml")).unwrap();
        assert_eq!(saved.display.chart_palette, palette);

        click_button(&harness, "Spending by category");
        harness.run_ok();
        let name = format!("{palette:?}").to_lowercase();
        maybe_render(&mut harness, &format!("palette-{name}"));
    }
}

#[test]
fn missing_config_opens_settings() {
    let dir = tempfile::tempdir().unwrap();
    let options = Options {
        config_path: dir.path().join("config.toml"),
        #[cfg(feature = "dev-mode")]
        screenshot: None,
    };
    let app = MoneyApp::new(options, None, today());
    let mut harness = harness(app);
    assert_eq!(harness.state().tab, Tab::Settings);
    click_button(&harness, "Transactions");
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
    let options = Options {
        config_path: dir.path().join("config.toml"),
        #[cfg(feature = "dev-mode")]
        screenshot: None,
    };
    let app = MoneyApp::new(options, None, today());
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
    click_button(&harness, "Open settings");
    harness.run_ok();
    assert_eq!(harness.state().tab, Tab::Settings);
}
