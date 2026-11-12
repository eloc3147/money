//! Editor for the TOML configuration.

use std::path::Path;

use eframe::egui::{self, RichText, Ui};
use egui_extras::DatePickerButton;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use strum::{IntoStaticStr, VariantArray};

use super::Action;
use super::range::{from_jiff, to_jiff};
use super::uncategorized::{enum_combo, mapping_grid};
use crate::config::{
    AccountConfig, AppConfig, BudgetConfig, BudgetRule, IncomeType, NameSource,
    TransactionRuleConfig, TransactionTypeConfig, TransactionTypeMode, UserTransactionType,
};

/// Sections of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, VariantArray, IntoStaticStr)]
pub enum Section {
    /// Accounts to import.
    #[default]
    Accounts,
    /// Transaction type mappings.
    #[strum(serialize = "Transaction types")]
    TransactionTypes,
    /// Categorization rules.
    Rules,
    /// Budgets.
    Budgets,
    /// Storage and raw TOML preview.
    Advanced,
}

/// State of the settings view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsView {
    /// The config as last loaded.
    pub original: AppConfig,
    /// The config being edited.
    pub draft: AppConfig,
    /// Visible section.
    pub section: Section,
    /// Rule filter text.
    pub rule_search: String,
}

impl SettingsView {
    /// Replace the edited config with a freshly loaded one, unless there are unsaved edits.
    pub fn set_config(&mut self, config: &AppConfig) {
        if !self.is_dirty() {
            self.draft = config.clone();
        }
        self.original = config.clone();
    }

    /// Whether the draft differs from the loaded config.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.draft != self.original
    }

    /// Draw the view. Returns [`Action::SaveConfig`] when the user saves.
    pub fn ui(&mut self, ui: &mut Ui, config_path: &Path) -> Option<Action> {
        let mut action = None;
        let problems = self.draft.validate();
        ui.horizontal_wrapped(|ui| {
            for &section in Section::VARIANTS {
                let label: &'static str = section.into();
                ui.selectable_value(&mut self.section, section, label);
            }
            ui.separator();
            let save = ui.add_enabled(
                self.is_dirty() && problems.is_empty(),
                egui::Button::new("💾 Save and reload"),
            );
            if save.clicked() {
                action = Some(Action::SaveConfig(Box::new(self.draft.clone())));
            }
            if ui
                .add_enabled(self.is_dirty(), egui::Button::new("↺ Revert"))
                .clicked()
            {
                self.draft = self.original.clone();
            }
            if self.is_dirty() {
                ui.colored_label(ui.visuals().warn_fg_color, "Unsaved changes");
            }
        });
        ui.weak(format!(
            "Editing {}. Saving rewrites the file (comments are not kept) and stores the previous \
             version as config.toml.bak.",
            config_path.display()
        ));
        for problem in &problems {
            ui.colored_label(ui.visuals().error_fg_color, format!("⚠ {problem}"));
        }
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| match self.section {
                Section::Accounts => accounts_ui(ui, &mut self.draft.account),
                Section::TransactionTypes => {
                    let names: Vec<String> =
                        self.draft.account.iter().map(|a| a.name.clone()).collect();
                    transaction_types_ui(ui, &mut self.draft.transaction_type, &names);
                }
                Section::Rules => rules_ui(ui, &mut self.draft.rule, &mut self.rule_search),
                Section::Budgets => budgets_ui(ui, &mut self.draft.budget),
                Section::Advanced => advanced_ui(ui, &mut self.draft),
            });
        action
    }
}

/// Add a text edit of a fixed width and `lines` rows (grids ignore a text edit's desired width).
fn sized(ui: &mut Ui, width: f32, lines: usize, edit: egui::TextEdit<'_>) -> egui::Response {
    #[allow(clippy::cast_precision_loss)]
    let height = ui.text_style_height(&egui::TextStyle::Body) * lines as f32
        + ui.spacing().button_padding.y * 2.0;
    ui.add_sized([width, height], edit)
}

/// A small "remove" button. Returns true when clicked.
fn remove_button(ui: &mut Ui) -> bool {
    ui.small_button("🗑").on_hover_text("Remove").clicked()
}

/// Edit the account list.
fn accounts_ui(ui: &mut Ui, accounts: &mut Vec<AccountConfig>) {
    ui.label("Each account's directory is searched recursively for .qfx and .csv statements.");
    ui.add_space(4.0);
    let mut remove = None;
    egui::Grid::new("accounts")
        .num_columns(3)
        .striped(true)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            let name_header = ui.strong("Name");
            let path_header = ui.strong("Statement directory");
            ui.end_row();
            for (idx, account) in accounts.iter_mut().enumerate() {
                sized(ui, 160.0, 1, egui::TextEdit::singleline(&mut account.name))
                    .labelled_by(name_header.id);
                let mut path = account.source_path.to_string_lossy().into_owned();
                if sized(ui, 420.0, 1, egui::TextEdit::singleline(&mut path))
                    .labelled_by(path_header.id)
                    .changed()
                {
                    account.source_path = path.into();
                }
                if remove_button(ui) {
                    remove = Some(idx);
                }
                ui.end_row();
            }
        });
    if let Some(idx) = remove {
        accounts.remove(idx);
    }
    if ui.button("➕ Add account").clicked() {
        accounts.push(AccountConfig {
            name: format!("Account {}", accounts.len() + 1),
            source_path: dirs::home_dir().unwrap_or_default(),
        });
    }
}

/// Edit the transaction type mappings.
fn transaction_types_ui(ui: &mut Ui, types: &mut Vec<TransactionTypeConfig>, accounts: &[String]) {
    ui.label(
        "Transaction types decide which raw transactions of an account a rule set applies to, \
         where the matching name comes from, and whether they count as income.",
    );
    ui.add_space(4.0);
    let mut remove = None;
    for (idx, mapping) in types.iter_mut().enumerate() {
        let title = match mapping.mode {
            TransactionTypeMode::Prefix => {
                format!("Prefix \"{}\"", mapping.prefix.as_deref().unwrap_or(""))
            }
            TransactionTypeMode::SourceType => format!(
                "Source type {}",
                mapping.source_type.map_or("?", |t| t.name())
            ),
        };
        egui::CollapsingHeader::new(format!(
            "{title}: {} ({})",
            mapping.transaction_type.name(),
            mapping.accounts.join(", ")
        ))
        .id_salt(("type", idx))
        .show(ui, |ui| {
            mapping_grid(ui, &format!("type-grid-{idx}"), mapping);
            ui.horizontal_wrapped(|ui| {
                ui.label("Applies to:");
                for account in accounts {
                    let mut on = mapping.accounts.contains(account);
                    if ui.checkbox(&mut on, account).changed() {
                        if on {
                            mapping.accounts.push(account.clone());
                        } else {
                            mapping.accounts.retain(|a| a != account);
                        }
                    }
                }
            });
            if ui.button("🗑 Remove type").clicked() {
                remove = Some(idx);
            }
        });
    }
    if let Some(idx) = remove {
        types.remove(idx);
    }
    if ui.button("➕ Add transaction type").clicked() {
        types.push(TransactionTypeConfig {
            mode: TransactionTypeMode::Prefix,
            prefix: Some(String::new()),
            source_type: None,
            transaction_type: UserTransactionType::DebitPurchase,
            income: IncomeType::No,
            name_source: NameSource::Name,
            accounts: accounts.to_vec(),
        });
    }
}

/// Edit the categorization rules.
fn rules_ui(ui: &mut Ui, rules: &mut Vec<TransactionRuleConfig>, search: &mut String) {
    ui.horizontal(|ui| {
        let label = ui.label("Filter:");
        ui.text_edit_singleline(search).labelled_by(label.id);
        ui.weak(format!("{} rules", rules.len()));
    });
    ui.weak("Patterns match the transaction name exactly (after trimming); one per line.");
    ui.add_space(4.0);
    let needle = search.trim().to_lowercase();
    let mut remove = None;
    egui::Grid::new("rules")
        .num_columns(5)
        .striped(true)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            let type_header = ui.strong("Type");
            let category_header = ui.strong("Category");
            let ignore_header = ui.strong("Ignore");
            let patterns_header = ui.strong("Patterns");
            ui.end_row();
            for (idx, rule) in rules.iter_mut().enumerate() {
                let visible = needle.is_empty()
                    || rule.category.to_lowercase().contains(&needle)
                    || rule
                        .patterns
                        .iter()
                        .any(|p| p.to_lowercase().contains(&needle));
                if !visible {
                    continue;
                }
                enum_combo(
                    ui,
                    type_header.id,
                    ("rule-type", idx),
                    &mut rule.transaction_type,
                    UserTransactionType::VARIANTS,
                );
                sized(ui, 200.0, 1, egui::TextEdit::singleline(&mut rule.category))
                    .labelled_by(category_header.id);
                ui.checkbox(&mut rule.ignore, "")
                    .labelled_by(ignore_header.id);
                let mut text = rule.patterns.join("\n");
                let lines = rule.patterns.len().max(1);
                if sized(
                    ui,
                    360.0,
                    lines,
                    egui::TextEdit::multiline(&mut text).desired_rows(lines),
                )
                .labelled_by(patterns_header.id)
                .changed()
                {
                    rule.patterns = text.lines().map(str::to_string).collect();
                }
                if remove_button(ui) {
                    remove = Some(idx);
                }
                ui.end_row();
            }
        });
    if let Some(idx) = remove {
        rules.remove(idx);
    }
    if ui.button("➕ Add rule").clicked() {
        search.clear();
        rules.push(TransactionRuleConfig {
            transaction_type: UserTransactionType::DebitPurchase,
            category: "New".into(),
            ignore: false,
            patterns: Vec::new(),
        });
    }
}

/// Edit the budgets.
fn budgets_ui(ui: &mut Ui, budgets: &mut Vec<BudgetConfig>) {
    ui.label("Each budget applies from its start date until the next budget starts.");
    ui.add_space(4.0);
    let mut remove = None;
    for (idx, budget) in budgets.iter_mut().enumerate() {
        egui::CollapsingHeader::new(format!("{} (from {})", budget.name, budget.start_date))
            .id_salt(("budget", idx))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let label = ui.label("Name");
                    ui.text_edit_singleline(&mut budget.name)
                        .labelled_by(label.id);
                    ui.label("Starts");
                    let mut date = to_jiff(budget.start_date);
                    ui.add(DatePickerButton::new(&mut date).id_salt(&format!("budget-{idx}")));
                    budget.start_date = from_jiff(date);
                });
                let mut remove_rule = None;
                egui::Grid::new(("budget-rules", idx))
                    .num_columns(3)
                    .show(ui, |ui| {
                        for (rule_idx, rule) in budget.rules.iter_mut().enumerate() {
                            sized(ui, 200.0, 1, egui::TextEdit::singleline(&mut rule.category));
                            let mut limit = rule.limit.to_f64().unwrap_or(0.0);
                            if ui
                                .add(
                                    egui::DragValue::new(&mut limit)
                                        .prefix("$")
                                        .speed(5.0)
                                        .range(0.0..=f64::MAX)
                                        .fixed_decimals(2),
                                )
                                .changed()
                            {
                                rule.limit =
                                    Decimal::try_from(limit).unwrap_or_default().round_dp(2);
                            }
                            if remove_button(ui) {
                                remove_rule = Some(rule_idx);
                            }
                            ui.end_row();
                        }
                    });
                if let Some(rule_idx) = remove_rule {
                    budget.rules.remove(rule_idx);
                }
                ui.horizontal(|ui| {
                    if ui.button("➕ Add limit").clicked() {
                        budget.rules.push(BudgetRule {
                            category: String::new(),
                            limit: Decimal::ZERO,
                        });
                    }
                    if ui.button("🗑 Remove budget").clicked() {
                        remove = Some(idx);
                    }
                });
            });
    }
    if let Some(idx) = remove {
        budgets.remove(idx);
    }
    if ui.button("➕ Add budget").clicked() {
        let start = budgets
            .last()
            .map_or_else(|| chrono::Local::now().date_naive(), |b| b.start_date);
        budgets.push(BudgetConfig {
            name: format!("Budget {}", budgets.len() + 1),
            start_date: start,
            rules: Vec::new(),
        });
    }
}

/// Storage settings and TOML preview.
fn advanced_ui(ui: &mut Ui, config: &mut AppConfig) {
    ui.horizontal(|ui| {
        let label = ui.label("Database file:");
        let mut path = config
            .database
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if ui
            .add(
                egui::TextEdit::singleline(&mut path)
                    .hint_text("default: data directory/money.sqlite")
                    .desired_width(420.0),
            )
            .labelled_by(label.id)
            .changed()
        {
            config.database.path = (!path.trim().is_empty()).then(|| path.into());
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("TOML preview").strong());
    let mut text = config.to_toml().unwrap_or_else(|e| format!("{e:#}"));
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .code_editor()
            .interactive(false)
            .desired_width(f32::INFINITY),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_dirty_state() {
        let mut view = SettingsView::default();
        let mut config = AppConfig::default();
        config.account.push(AccountConfig {
            name: "A".into(),
            source_path: "/a".into(),
        });
        view.set_config(&config);
        assert!(!view.is_dirty());
        assert_eq!(view.draft, config);

        view.draft.account.clear();
        assert!(view.is_dirty());

        // A reload must not clobber unsaved edits.
        view.set_config(&config);
        assert!(view.draft.account.is_empty());
        assert!(view.is_dirty());
    }
}
