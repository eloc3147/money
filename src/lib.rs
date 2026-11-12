//! Money: a simple personal finance tracker.
//!
//! Bank statements (QFX and CSV) are imported from configured account directories into a
//! SQLite database on every start, categorized by rules from a TOML config, and shown in a
//! native [`egui`] GUI.
//!
//! Module overview:
//! - [`config`]: the TOML configuration and helpers to edit it.
//! - [`importer`]: statement parsing and categorization.
//! - [`db`]: SQLite storage.
//! - [`loader`]: the background load pipeline tying the above together.
//! - [`model`]: records read back for display.
//! - [`analysis`]: pure aggregation used by the views.
//! - [`error`]: structured, colorless error reports passed out of background work.
//! - [`gui`]: the desktop application.
//! - [`mock`]: deterministic mock data for testing and demos.

#![deny(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

pub mod analysis;
pub mod config;
pub mod db;
pub mod error;
pub mod gui;
pub mod importer;
pub mod loader;
pub mod mock;
pub mod model;
