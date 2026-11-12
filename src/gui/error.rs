//! Display of an [`ErrorReport`]: message, cause chain, location and a stack trace table.

use eframe::egui::{self, Color32, RichText, Ui};

use crate::error::{ErrorReport, Frame, SourceLocation};

/// State of the error display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ErrorView {
    /// Show frames from std and dependencies as well as application frames.
    pub show_all_frames: bool,
}

/// Shorten a source path for display: paths inside a crate keep the part from `src/` on, so
/// `/home/me/.cargo/registry/.../tokio-1.0/src/runtime/park.rs` becomes
/// `tokio-1.0/src/runtime/park.rs`.
#[must_use]
pub fn short_path(file: &str) -> String {
    let file = file.replace('\\', "/");
    match file.rfind("/src/") {
        Some(idx) => {
            let crate_start = file[..idx].rfind('/').map_or(0, |i| i + 1);
            file[crate_start..].to_string()
        }
        None => file,
    }
}

/// Format a location with a shortened path.
fn short_location(location: &SourceLocation) -> String {
    let mut text = format!("{}:{}", short_path(&location.file), location.line);
    if let Some(column) = location.column {
        text.push_str(&format!(":{column}"));
    }
    text
}

impl ErrorView {
    /// Draw `report`.
    pub fn ui(&mut self, ui: &mut Ui, report: &ErrorReport) {
        let error_color = ui.visuals().error_fg_color;
        egui::Frame::group(ui.style())
            .stroke(egui::Stroke::new(1.0, error_color))
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(&report.message)
                        .size(16.0)
                        .strong()
                        .color(error_color),
                );
                if let Some(location) = &report.location {
                    ui.label(
                        RichText::new(format!("at {}", short_location(location)))
                            .monospace()
                            .weak(),
                    )
                    .on_hover_text(location.to_string());
                }
                if !report.causes.is_empty() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Caused by").strong());
                    egui::Grid::new("error-causes")
                        .num_columns(2)
                        .spacing([8.0, 4.0])
                        .show(ui, |ui| {
                            for (idx, cause) in report.causes.iter().enumerate() {
                                ui.label(RichText::new(format!("{idx}.")).monospace().weak());
                                ui.add(egui::Label::new(cause).wrap());
                                ui.end_row();
                            }
                        });
                }
                ui.add_space(6.0);
                if ui
                    .button("Copy report")
                    .on_hover_text("Copy as plain text")
                    .clicked()
                {
                    ui.ctx().copy_text(report.to_string());
                }
            });

        ui.add_space(8.0);
        let app_count = report.app_frames().count();
        if report.frames.is_empty() {
            ui.weak("No stack trace was captured for this error.");
            return;
        }
        egui::CollapsingHeader::new(format!("Stack trace ({app_count} application frames)"))
            .id_salt("error-stack")
            .default_open(true)
            .show(ui, |ui| {
                ui.checkbox(
                    &mut self.show_all_frames,
                    format!(
                        "Show all {} frames (std and dependencies)",
                        report.frames.len()
                    ),
                );
                let frames: Vec<(usize, &Frame)> = report
                    .frames
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| self.show_all_frames || f.is_app())
                    .collect();
                frame_table(ui, &frames);
            });
    }
}

/// A table of stack frames (function, then source location below it), application frames
/// emphasised.
fn frame_table(ui: &mut Ui, frames: &[(usize, &Frame)]) {
    let app_color = if ui.visuals().dark_mode {
        Color32::from_rgb(0x86, 0xb6, 0xef)
    } else {
        Color32::from_rgb(0x1c, 0x5c, 0xab)
    };
    egui::Grid::new("error-frames")
        .num_columns(2)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for (idx, frame) in frames {
                ui.label(RichText::new(idx.to_string()).monospace().weak());
                ui.vertical(|ui| {
                    // Generic names can be very long: truncate, full name on hover.
                    let name = RichText::new(&frame.function).monospace();
                    let name = if frame.is_app() {
                        name.color(app_color).strong()
                    } else {
                        name.weak()
                    };
                    ui.add(egui::Label::new(name).truncate());
                    if let Some(location) = &frame.location {
                        ui.label(
                            RichText::new(format!("at {}", short_location(location)))
                                .small()
                                .monospace()
                                .weak(),
                        )
                        .on_hover_text(location.to_string());
                    }
                });
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_paths() {
        assert_eq!(short_path("src/loader.rs"), "src/loader.rs");
        assert_eq!(
            short_path("/home/me/.cargo/registry/src/index/tokio-1.0/src/runtime/park.rs"),
            "tokio-1.0/src/runtime/park.rs"
        );
        assert_eq!(short_path("C:\\work\\money\\src\\db.rs"), "money/src/db.rs");
        assert_eq!(
            short_path("/rustc/abc/library/core/src/ops.rs"),
            "core/src/ops.rs"
        );
        assert_eq!(short_path("main.rs"), "main.rs");
    }
}
