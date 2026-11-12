//! Uncategorized transactions, grouped by name, with forms to add rules or type mappings.

use eframe::egui::{self, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use strum::VariantArray;

use super::Action;
use super::style::{amount_label, money};
use crate::analysis::{UncategorizedGroup, group_uncategorized};
use crate::config::{
    AppConfig, IncomeType, NameSource, TransactionTypeConfig, TransactionTypeMode,
    UserTransactionType,
};
use crate::importer::TransactionType;
use crate::model::{Dataset, UncategorizedKind};

/// Form for adding a categorization rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleForm {
    /// Transaction type the rule applies to.
    pub transaction_type: UserTransactionType,
    /// Exact name to match.
    pub pattern: String,
    /// Category to assign.
    pub category: String,
    /// Drop matching transactions instead.
    pub ignore: bool,
}

impl RuleForm {
    /// Apply the form to `config`, or explain why it can't be applied.
    ///
    /// # Errors
    /// Returns a message if the pattern or category is empty.
    pub fn apply(&self, config: &mut AppConfig) -> Result<(), String> {
        if self.pattern.trim().is_empty() {
            return Err("Pattern must not be empty".into());
        }
        if self.category.trim().is_empty() {
            return Err("Category must not be empty".into());
        }
        config.add_rule_pattern(
            self.transaction_type,
            self.category.trim(),
            self.ignore,
            &self.pattern,
        );
        Ok(())
    }
}

/// Form for adding a transaction type mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingForm {
    /// The mapping being built.
    pub mapping: TransactionTypeConfig,
}

impl MappingForm {
    /// Apply the form to `config`, or explain why it can't be applied.
    ///
    /// # Errors
    /// Returns a message if the mapping is incomplete.
    pub fn apply(&self, config: &mut AppConfig) -> Result<(), String> {
        let mut mapping = self.mapping.clone();
        match mapping.mode {
            TransactionTypeMode::Prefix => {
                mapping.source_type = None;
                if mapping.prefix.as_deref().unwrap_or("").is_empty() {
                    return Err("Prefix must not be empty".into());
                }
            }
            TransactionTypeMode::SourceType => {
                mapping.prefix = None;
                if mapping.name_source == NameSource::NameSuffix {
                    return Err("Name suffix requires Prefix mode".into());
                }
            }
        }
        config.transaction_type.push(mapping);
        Ok(())
    }
}

/// The form for the selected group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Form {
    /// Add a rule.
    Rule(RuleForm),
    /// Add a type mapping.
    Mapping(MappingForm),
}

impl Form {
    /// A form prefilled from `group`.
    #[must_use]
    pub fn for_group(group: &UncategorizedGroup) -> Self {
        match group.kind {
            UncategorizedKind::MissingRule(transaction_type) => Self::Rule(RuleForm {
                transaction_type,
                pattern: group.message.clone(),
                category: String::new(),
                ignore: false,
            }),
            UncategorizedKind::MissingType(source_type) => Self::Mapping(MappingForm {
                mapping: TransactionTypeConfig {
                    mode: TransactionTypeMode::SourceType,
                    prefix: Some(
                        group
                            .message
                            .split_whitespace()
                            .next()
                            .map(|w| format!("{w} "))
                            .unwrap_or_default(),
                    ),
                    source_type: Some(source_type),
                    transaction_type: UserTransactionType::DebitPurchase,
                    income: IncomeType::No,
                    name_source: NameSource::Name,
                    accounts: vec![group.account.clone()],
                },
            }),
        }
    }
}

/// State of the uncategorized view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UncategorizedView {
    /// Text filter.
    pub search: String,
    /// Selected group key: (kind, account, message).
    pub selected: Option<(UncategorizedKind, String, String)>,
    /// Form for the selected group.
    pub form: Option<Form>,
    /// Last validation error.
    pub error: Option<String>,
}

impl UncategorizedView {
    /// Groups matching the search text.
    #[must_use]
    pub fn groups(&self, data: &Dataset) -> Vec<UncategorizedGroup> {
        let needle = self.search.trim().to_lowercase();
        group_uncategorized(&data.uncategorized)
            .into_iter()
            .filter(|g| {
                needle.is_empty()
                    || g.message.to_lowercase().contains(&needle)
                    || g.account.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// Select a group and prefill its form.
    pub fn select(&mut self, group: &UncategorizedGroup) {
        self.selected = Some((group.kind, group.account.clone(), group.message.clone()));
        self.form = Some(Form::for_group(group));
        self.error = None;
    }

    /// Apply the current form to a copy of `config`.
    ///
    /// # Errors
    /// Returns a message if there is no form or it is invalid.
    pub fn submit(&mut self, config: &AppConfig) -> Result<AppConfig, String> {
        let mut config = config.clone();
        match &self.form {
            Some(Form::Rule(form)) => form.apply(&mut config)?,
            Some(Form::Mapping(form)) => form.apply(&mut config)?,
            None => return Err("Select a transaction first".into()),
        }
        self.selected = None;
        self.form = None;
        self.error = None;
        Ok(config)
    }

    /// Draw the view. Returns an action when the user adds a rule or mapping.
    pub fn ui(&mut self, ui: &mut Ui, data: &Dataset, config: &AppConfig) -> Option<Action> {
        let groups = self.groups(data);
        let mut action = None;

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} uncategorized transactions in {} groups",
                    data.uncategorized.len(),
                    group_uncategorized(&data.uncategorized).len()
                ))
                .strong(),
            );
            ui.separator();
            let label = ui.label("Search:");
            ui.text_edit_singleline(&mut self.search)
                .labelled_by(label.id);
        });
        ui.add_space(4.0);

        if data.uncategorized.is_empty() {
            ui.centered_and_justified(|ui| ui.label("🎉 Every transaction is categorized."));
            return None;
        }

        egui::Panel::right("uncategorized-form")
            .resizable(true)
            .default_size(340.0)
            .show(ui, |ui| {
                action = self.form_ui(ui, config);
            });

        egui::CentralPanel::no_frame().show(ui, |ui| self.table(ui, &groups));
        action
    }

    /// Draw the groups table.
    fn table(&mut self, ui: &mut Ui, groups: &[UncategorizedGroup]) {
        let row_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
        // Selectable labels would swallow clicks meant for selecting the row.
        ui.style_mut().interaction.selectable_labels = false;
        TableBuilder::new(ui)
            .id_salt("uncategorized-table")
            .striped(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(50.0))
            .column(Column::initial(100.0))
            .column(Column::initial(220.0))
            .column(Column::initial(240.0).clip(true))
            .column(Column::initial(110.0))
            .column(Column::remainder())
            .header(row_height, |mut header| {
                for name in ["Count", "Account", "Reason", "Name", "Total", "Dates"] {
                    header.col(|ui| {
                        ui.strong(name);
                    });
                }
            })
            .body(|body| {
                body.rows(row_height, groups.len(), |mut row| {
                    let group = &groups[row.index()];
                    let selected = self.selected.as_ref().is_some_and(|(k, a, m)| {
                        *k == group.kind && *a == group.account && *m == group.message
                    });
                    row.set_selected(selected);
                    row.col(|ui| {
                        ui.label(group.count.to_string());
                    });
                    row.col(|ui| {
                        ui.label(&group.account);
                    });
                    row.col(|ui| {
                        ui.label(group.kind.label());
                    });
                    row.col(|ui| {
                        ui.label(&group.message);
                    });
                    row.col(|ui| amount_label(ui, group.total));
                    row.col(|ui| {
                        if group.first == group.last {
                            ui.label(group.first.to_string());
                        } else {
                            ui.label(format!("{} – {}", group.first, group.last));
                        }
                    });
                    if row.response().clicked() {
                        self.select(group);
                    }
                });
            });
    }

    /// Draw the form for the selected group.
    fn form_ui(&mut self, ui: &mut Ui, config: &AppConfig) -> Option<Action> {
        let Some(form) = &mut self.form else {
            ui.add_space(8.0);
            ui.weak("Select a row to add a rule for it.");
            return None;
        };
        let categories = config.categories();
        let submit = match form {
            Form::Rule(rule) => {
                ui.heading("Add rule");
                ui.weak(
                    "Transactions of this type whose name matches the pattern exactly are \
                     assigned the category.",
                );
                ui.add_space(6.0);
                egui::Grid::new("rule-form")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Type");
                        ui.label(rule.transaction_type.name());
                        ui.end_row();
                        let label = ui.label("Pattern");
                        ui.text_edit_singleline(&mut rule.pattern)
                            .labelled_by(label.id);
                        ui.end_row();
                        let label = ui.label("Category");
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut rule.category)
                                    .hint_text("e.g. Food.Groceries")
                                    .desired_width(150.0),
                            )
                            .labelled_by(label.id);
                            egui::ComboBox::from_id_salt("rule-category-pick")
                                .selected_text("existing…")
                                .width(90.0)
                                .show_ui(ui, |ui| {
                                    for category in &categories {
                                        if ui
                                            .selectable_label(*category == rule.category, category)
                                            .clicked()
                                        {
                                            rule.category.clone_from(category);
                                        }
                                    }
                                })
                                .response
                                .labelled_by(label.id);
                        });
                        ui.end_row();
                        ui.label("Ignore");
                        ui.checkbox(&mut rule.ignore, "Drop matching transactions");
                        ui.end_row();
                    });
                ui.add_space(6.0);
                ui.button("Add rule and reload").clicked()
            }
            Form::Mapping(form) => {
                ui.heading("Add transaction type");
                ui.weak(
                    "No transaction type matches these transactions. A type decides how the \
                     name is read and whether it is income.",
                );
                ui.add_space(6.0);
                mapping_grid(ui, "mapping-form", &mut form.mapping);
                ui.add_space(6.0);
                ui.button("Add type and reload").clicked()
            }
        };
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if submit {
            match self.submit(config) {
                Ok(config) => return Some(Action::SaveConfig(Box::new(config))),
                Err(error) => self.error = Some(error),
            }
        }
        None
    }
}

/// Editable fields of a transaction type mapping (shared with the settings view).
pub fn mapping_grid(ui: &mut Ui, id: &str, mapping: &mut TransactionTypeConfig) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            let label = ui.label("Mode");
            enum_combo(
                ui,
                label.id,
                (id, "mode"),
                &mut mapping.mode,
                TransactionTypeMode::VARIANTS,
            );
            ui.end_row();
            match mapping.mode {
                TransactionTypeMode::Prefix => {
                    let label = ui.label("Prefix");
                    let prefix = mapping.prefix.get_or_insert_with(String::new);
                    ui.text_edit_singleline(prefix).labelled_by(label.id);
                }
                TransactionTypeMode::SourceType => {
                    let label = ui.label("Source type");
                    let source = mapping.source_type.get_or_insert(TransactionType::Debit);
                    enum_combo(
                        ui,
                        label.id,
                        (id, "source"),
                        source,
                        TransactionType::VARIANTS,
                    );
                }
            }
            ui.end_row();
            let label = ui.label("Type");
            enum_combo(
                ui,
                label.id,
                (id, "type"),
                &mut mapping.transaction_type,
                UserTransactionType::VARIANTS,
            );
            ui.end_row();
            let label = ui.label("Income");
            enum_combo(
                ui,
                label.id,
                (id, "income"),
                &mut mapping.income,
                IncomeType::VARIANTS,
            );
            ui.end_row();
            let label = ui.label("Name source");
            enum_combo(
                ui,
                label.id,
                (id, "name"),
                &mut mapping.name_source,
                NameSource::VARIANTS,
            );
            ui.end_row();
            ui.label("Accounts");
            ui.label(mapping.accounts.join(", "));
            ui.end_row();
        });
}

/// A combo box over every variant of an enum, named for accessibility by the label `labelled_by`.
pub fn enum_combo<T>(
    ui: &mut Ui,
    labelled_by: egui::Id,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: &mut T,
    variants: &[T],
) where
    T: Copy + PartialEq + Into<&'static str>,
{
    egui::ComboBox::from_id_salt(id)
        .selected_text(Into::<&'static str>::into(*value))
        .show_ui(ui, |ui| {
            for &variant in variants {
                ui.selectable_value(value, variant, Into::<&'static str>::into(variant));
            }
        })
        .response
        .labelled_by(labelled_by);
}

/// Summary text for a group, used in tests and tooltips.
#[must_use]
pub fn describe(group: &UncategorizedGroup) -> String {
    format!(
        "{} × {} ({}) {}",
        group.count,
        group.message,
        group.account,
        money(group.total)
    )
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    use super::*;
    use crate::model::UncategorizedRecord;

    fn data() -> Dataset {
        let rec = |kind, message: &str| UncategorizedRecord {
            id: 0,
            kind,
            account: "Chequing".into(),
            message: message.into(),
            posted_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            amount: Decimal::new(-100, 2),
        };
        Dataset {
            transactions: vec![],
            uncategorized: vec![
                rec(
                    UncategorizedKind::MissingRule(UserTransactionType::DebitPurchase),
                    "CAFE",
                ),
                rec(
                    UncategorizedKind::MissingRule(UserTransactionType::DebitPurchase),
                    "CAFE",
                ),
                rec(
                    UncategorizedKind::MissingType(TransactionType::Atm),
                    "ATM WITHDRAWAL",
                ),
            ],
        }
    }

    #[test]
    fn search_filters_groups() {
        let mut view = UncategorizedView::default();
        assert_eq!(view.groups(&data()).len(), 2);
        view.search = "atm".into();
        let groups = view.groups(&data());
        assert_eq!(groups.len(), 1);
        assert_eq!(describe(&groups[0]), "1 × ATM WITHDRAWAL (Chequing) -$1.00");
    }

    #[test]
    fn rule_form_adds_rule() {
        let mut view = UncategorizedView::default();
        let groups = view.groups(&data());
        view.select(&groups[0]);
        let Some(Form::Rule(rule)) = &mut view.form else {
            panic!("expected rule form")
        };
        assert_eq!(rule.pattern, "CAFE");
        assert_eq!(
            view.submit(&AppConfig::default()),
            Err("Category must not be empty".into())
        );

        let Some(Form::Rule(rule)) = &mut view.form else {
            panic!("form should survive a failed submit")
        };
        rule.category = "Food.Coffee".into();
        let config = view.submit(&AppConfig::default()).unwrap();
        assert_eq!(config.rule.len(), 1);
        assert_eq!(config.rule[0].patterns, vec!["CAFE"]);
        assert!(view.form.is_none());
        assert!(view.submit(&config).is_err(), "no form selected");
    }

    #[test]
    fn rule_form_rejects_empty_pattern() {
        let form = RuleForm {
            transaction_type: UserTransactionType::Interest,
            pattern: " ".into(),
            category: "X".into(),
            ignore: false,
        };
        assert!(form.apply(&mut AppConfig::default()).is_err());
    }

    #[test]
    fn mapping_form_adds_mapping() {
        let mut view = UncategorizedView::default();
        let groups = view.groups(&data());
        view.select(&groups[1]);
        let Some(Form::Mapping(form)) = &mut view.form else {
            panic!("expected mapping form")
        };
        assert_eq!(form.mapping.source_type, Some(TransactionType::Atm));
        assert_eq!(form.mapping.prefix.as_deref(), Some("ATM "));
        form.mapping.transaction_type = UserTransactionType::AtmWithdrawal;
        let config = view.submit(&AppConfig::default()).unwrap();
        assert_eq!(config.transaction_type.len(), 1);
        assert_eq!(
            config.transaction_type[0].prefix, None,
            "prefix dropped in SourceType mode"
        );
    }

    #[test]
    fn mapping_form_validates() {
        let mut form = MappingForm {
            mapping: TransactionTypeConfig {
                mode: TransactionTypeMode::Prefix,
                prefix: Some(String::new()),
                source_type: Some(TransactionType::Atm),
                transaction_type: UserTransactionType::AtmWithdrawal,
                income: IncomeType::No,
                name_source: NameSource::NameSuffix,
                accounts: vec![],
            },
        };
        assert!(form.apply(&mut AppConfig::default()).is_err());
        form.mapping.prefix = Some("ATM ".into());
        let mut config = AppConfig::default();
        form.apply(&mut config).unwrap();
        assert_eq!(config.transaction_type[0].source_type, None);

        form.mapping.mode = TransactionTypeMode::SourceType;
        assert!(form.apply(&mut config).is_err(), "suffix needs prefix mode");
    }
}
