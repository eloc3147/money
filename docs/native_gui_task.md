# Native GUI migration — task & progress log

This document tracks the conversion of Money from a "import into Postgres, view in
Grafana" tool into a self-contained native desktop application.

## Requirements

| # | Requirement | Status |
|---|-------------|--------|
| 1 | Docs for all items, plus a check that enforces doc coverage | ☐ |
| 2 | Unit tests, plus code coverage measurement in CI | ☐ |
| 3 | Non-web GUI framework (no Electron/webviews) supporting Windows and Linux | ☐ |
| 4 | SQLite as the transaction data store (replacing Postgres) | ☐ |
| 5 | Config stays in TOML, with a UI for editing it | ☐ |
| 6 | Rich graphs and tables comparable to a good finance program | ☐ |
| 7 | Transactions are (re)loaded from the source files each time the program starts | ☐ |
| 8 | GPU rendering if not significantly more complex | ☐ |
| 9 | GUI tested on Linux (Windows testing not possible in this environment) | ☐ |
| 10 | Mock data for testing | ☐ |
| 11 | Progress tracked in git and in this document | ☐ |
| 12 | View: transaction list, split by income / expense | ☐ |
| 13 | View: stacked bar graph of expenses by category, configurable range and averaging/grouping | ☐ |
| 14 | View: uncategorized transactions, with options for adding new rules | ☐ |
| 15 | No hand-written lists of enum values (variant arrays, name tables); derive them with a macro library | ☐ |

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
- **Doc coverage:** `#![deny(missing_docs)]` plus `clippy::missing_docs_in_private_items`
  are enabled crate-wide, so CI (clippy with `-D warnings`) fails on any undocumented item.
- **Code coverage:** `cargo llvm-cov` in CI with a minimum line-coverage threshold.

## Progress log

- 2026-11-11 — Surveyed existing code; chose egui/eframe + sqlx/sqlite; wrote this plan.
