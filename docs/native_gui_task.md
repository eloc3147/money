# Native GUI migration — task & progress log

This document tracks the conversion of Money from a "import into Postgres, view in
Grafana" tool into a self-contained native desktop application.

## Requirements

| # | Requirement | Status |
|---|-------------|--------|
| 1 | Docs for all items, plus a check that enforces doc coverage | ✅ |
| 2 | Unit tests, plus code coverage measurement in CI | ✅ |
| 3 | Non-web GUI framework (no Electron/webviews) supporting Windows and Linux | ✅ |
| 4 | SQLite as the transaction data store (replacing Postgres) | ✅ |
| 5 | Config stays in TOML, with a UI for editing it | ✅ |
| 6 | Rich graphs and tables comparable to a good finance program | ✅ |
| 7 | Transactions are (re)loaded from the source files each time the program starts | ✅ |
| 8 | GPU rendering if not significantly more complex | ✅ |
| 9 | GUI tested on Linux (Windows testing not possible in this environment) | ✅ Linux (Windows: CI builds and runs headless tests only) |
| 10 | Mock data for testing | ✅ |
| 11 | Progress tracked in git and in this document | ✅ |
| 12 | View: transaction list, split by income / expense | ✅ |
| 13 | View: stacked bar graph of expenses by category, configurable range and averaging/grouping | ✅ |
| 14 | View: uncategorized transactions, with options for adding new rules | ✅ |
| 15 | No hand-written lists of enum values (variant arrays, name tables); derive them with a macro library | ✅ |
| 16 | Errors from background work are returned in an easily parsable, colorless format and rendered in the GUI as a readable stack trace | ✅ |

## Design decisions

- **GUI framework: [egui](https://github.com/emilk/egui) via `eframe`.** Pure Rust,
  immediate-mode, renders natively through `wgpu` (Vulkan/Metal/DX12/GL) on Windows,
  Linux and macOS. `egui_plot` provides stacked bar charts with legends and hover
  tooltips, `egui_extras` provides virtualised, resizable tables and a date picker.
  `egui_kittest` allows the GUI to be driven and rendered headlessly in tests.
  Alternatives considered: `iced` (fewer charting/table widgets), `slint` (separate
  markup language, licensing), Qt/GTK bindings (heavy native dependencies, harder
  Windows builds).
- **GPU rendering:** eframe's `wgpu` backend. It costs nothing extra in code; it falls
  back to software rasterisers (e.g. lavapipe) where no GPU is present.
- **Storage: SQLite through `sqlx`.** The existing importer was already async `sqlx`
  code, so switching the driver keeps the importer structure intact. Amounts are stored
  as integer cents (SQLite has no exact decimal type). The database file is
  `money.sqlite` in the data directory and is rebuilt from the source files on every
  start (and whenever the config is changed from the UI).
- **Data flow:** a background thread with its own Tokio runtime loads the config,
  builds the categorizer, imports all files into SQLite and then reads back a
  [`Dataset`](../src/model.rs) snapshot. All views filter/aggregate that in-memory
  snapshot, so the UI never blocks on I/O. Aggregation logic lives in
  [`analysis`](../src/analysis.rs) as pure, unit-tested functions.
- **Config editing:** the config structs gained `Serialize`. Saving from the UI writes
  a `config.toml.bak` backup first. Comments in a hand-written config are not preserved
  by the serializer — this is called out in the settings UI.
- **Enum metadata via `strum`:** variant lists (`VariantArray::VARIANTS`), names
  (`IntoStaticStr`) and parsing (`EnumString`) are derived, never written out by hand, so
  adding a variant can't leave a drop-down or name table stale. (Added mid-task at the
  user's request.)
- **Error reports (requirement 16, added after the first PR push):** `error::install_hook`
  replaces color-eyre's report hook with a plain-text one that records where an error was
  raised and captures a backtrace. Background work hands the GUI an
  [`ErrorReport`](../src/error.rs) (message, cause chain, source location, stack frames; serde
  serializable) instead of a pre-formatted string, so no ANSI color codes reach the UI. The
  GUI shows the message, numbered causes and location, a "Copy report" button, and a stack
  table with application frames highlighted (std/dependency frames behind a toggle).
  color-eyre's panic hook is kept for terminal panics. The location is where the error was
  *first* raised, which points at the real cause rather than the outermost `wrap_err`.
- **Doc coverage:** `#![deny(missing_docs)]` plus `clippy::missing_docs_in_private_items`
  are enabled crate-wide, so CI (clippy with `-D warnings`) fails on any undocumented item.
- **Code coverage:** `cargo llvm-cov` in CI with a minimum line-coverage threshold.

## Progress log

- 2026-11-11 — Surveyed existing code; chose egui/eframe + sqlx/sqlite; wrote this plan.
- 2026-11-11 — Backend: Postgres replaced with SQLite (amounts as integer cents); categorizer
  owns its data (no more leaked `'static` config); indicatif progress bars replaced by
  `ImportProgress` counters; background `loader`; pure `analysis` module; deterministic mock
  data generator writing real QFX (XML) and CSV files. Enum lists derived with `strum` (new
  requirement 15).
- 2026-11-11 — GUI: transactions, spending chart, uncategorized and settings views; `--mock`,
  `--headless`, `--tab` and hidden `--screenshot` flags. Headless GUI tests with
  `egui_kittest` (tab switching, filters, chart controls, add-rule → save → reload, settings
  save/revert, missing config, import errors). kittest's accessibility check caught unlabeled
  combo boxes/text fields; they are now labelled.
- 2026-11-11 — Docs on every item (enforced by lints), QFX SGML/header tests, CI jobs for
  doc coverage, Linux + Windows tests and an 85% line-coverage gate (currently ~90%).

- 2026-11-11 — Requirement 16: load errors were shown as the raw color-eyre terminal output
  (all red, with escape codes). Added `ErrorReport` + plain eyre hook, loader returns
  structured reports, and a GUI error view with cause chain and stack trace. Hook-dependent
  tests live in `tests/error_report.rs` (their own process, so the hook is installed before
  any report is created).

## Testing notes

- `cargo test --all-targets` runs 65 unit tests, 2 error-report tests and 8 GUI tests (no
  display needed). Resolving backtraces makes the error-report tests take ~15 s in debug
  builds.
- The real window was launched on Linux (Wayland, VMware SVGA VM) with mock data and captured
  with `--screenshot` on every tab. With `WGPU_BACKEND=vulkan` (lavapipe) captures are
  correct. With the default adapter on this VM (OpenGL on VMware SVGA) the captured frame is
  blank grey; whether that is only the read-back or also the on-screen image could not be
  checked from this environment.
- Windows has not been run interactively; CI builds and runs the test suite on
  `windows-latest`.

## Known limitations / follow-ups

- Saving the config from the UI drops comments and reorders keys (`toml` serializer). A
  format-preserving writer (`toml_edit`) would fix this.
- Budgets are stored in the database but not yet shown in any view.
- All transactions are re-imported on every start and after every config change, as required
  "for now"; incremental import would need change detection on the source files.
- The legacy `grafana/` dashboard was removed: it queried the Postgres schema, which no longer
  exists.
