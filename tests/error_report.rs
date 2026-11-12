//! Error reports carry a location and stack frames once the hook is installed.
//!
//! This is its own test binary so the hook is installed before any other code creates an
//! `eyre::Report` (eyre otherwise auto-installs its default handler).

use color_eyre::eyre::{self, WrapErr, eyre};
use money::error::{ErrorReport, install_hook};
use money::loader::{LoadOptions, spawn_load};

#[inline(never)]
fn failing() -> eyre::Result<()> {
    Err(eyre!("inner problem"))
        .wrap_err("middle")
        .wrap_err("outer")
}

#[test]
fn reports_have_location_and_frames() {
    install_hook();
    let report = ErrorReport::from(failing().unwrap_err());
    assert_eq!(report.causes, vec!["middle", "inner problem"]);
    let location = report.location.as_ref().expect("location captured");
    assert!(location.file.ends_with("error_report.rs"), "{location}");
    assert!(
        report.frames.iter().any(|f| f.function.contains("failing")),
        "test frame present: {report:#?}"
    );

    let text = report.to_string();
    assert!(text.contains("\n\nLocation: "), "{text}");
    assert!(!text.contains('\u{1b}'), "no ANSI escapes");

    // `Debug` of the report (what `main` prints on error) is the same plain text.
    let debug = format!("{:?}", failing().unwrap_err());
    assert!(debug.starts_with("outer\n\nCaused by:"), "{debug}");
    assert!(!debug.contains('\u{1b}'));
}

#[test]
fn loader_errors_carry_app_frames() {
    install_hook();
    let dir = tempfile::tempdir().unwrap();
    let report = spawn_load(
        LoadOptions {
            config_path: dir.path().join("missing.toml"),
            default_db_path: None,
        },
        || {},
    )
    .wait()
    .unwrap_err();
    assert_eq!(report.message, "Failed to load config");
    // The location is where the error was first raised, not where it was last wrapped.
    assert!(
        report.location.as_ref().unwrap().file.contains("config.rs"),
        "{report}"
    );
    assert!(
        report
            .app_frames()
            .any(|f| f.function.starts_with("money::loader::")),
        "{report:#?}"
    );
    assert!(
        report
            .app_frames()
            .all(|f| !f.function.starts_with("money::error::"))
    );
}
