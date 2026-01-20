use crate::app::{MessageType, TaxApp, Screen};
use egui::Ui;
use rust_decimal::Decimal;

pub struct SelfEmploymentScreen;

impl SelfEmploymentScreen {
    pub fn show(app: &mut TaxApp, ui: &mut Ui) {
        ui.heading("Self-Employment Tax Worksheet");
        ui.separator();

        ui.label("Complete this section if you have self-employment income.");
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Use a fixed width for consistent group sizing
            let group_width = ui.available_width().min(500.0);

            ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                ui.group(|ui| {
                    ui.set_min_width(group_width - 20.0);
                    ui.heading("Self-Employment Income");
                    ui.add_space(5.0);

                    Self::currency_grid(ui, "se_income_grid", |ui| {
                        Self::grid_currency_row(
                            ui,
                            "Net Self-Employment Income:",
                            &mut app.form.se_income,
                            true,
                        );
                    });

                    ui.add_space(5.0);
                    ui.label("Enter your expected net profit from Schedule C, Schedule F, or K-1.");
                });
            });

            ui.add_space(10.0);

            ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                ui.group(|ui| {
                    ui.set_min_width(group_width - 20.0);
                    ui.heading("Additional SE Information");
                    ui.add_space(5.0);

                    Self::currency_grid(ui, "se_additional_grid", |ui| {
                        Self::grid_currency_row(
                            ui,
                            "CRP Payments:",
                            &mut app.form.expected_crp_payments,
                            false,
                        );
                        Self::grid_currency_row(
                            ui,
                            "Wages (if also employed):",
                            &mut app.form.expected_wages,
                            false,
                        );
                    });

                    ui.add_space(5.0);
                    ui.label("Wages affect the Social Security portion of SE tax calculation.");
                });
            });

            // Show SE-specific validation errors
            if !app.form.errors.is_empty() {
                ui.add_space(10.0);
                ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                    ui.group(|ui| {
                        ui.set_min_width(group_width - 20.0);
                        ui.colored_label(egui::Color32::RED, "Validation Errors:");
                        for error in &app.form.errors {
                            ui.colored_label(egui::Color32::RED, format!("  • {error}"));
                        }
                    });
                });
            }

            ui.add_space(20.0);

            // SE Tax Results
            ui.allocate_ui(egui::vec2(group_width, 0.0), |ui| {
                ui.group(|ui| {
                    ui.set_min_width(group_width - 20.0);
                    ui.heading("SE Tax Results");
                    ui.add_space(5.0);

                    if let Some(se_tax) = app.results.se_tax {
                        egui::Grid::new("se_results")
                            .num_columns(2)
                            .spacing([40.0, 8.0])
                            .show(ui, |ui| {
                                ui.label("Estimated SE Tax:");
                                ui.strong(format!("${:.2}", se_tax));
                                ui.end_row();

                                let deductible = se_tax / Decimal::TWO;
                                ui.label("Deductible Portion (50%):");
                                ui.label(format!("${:.2}", deductible));
                                ui.end_row();
                            });
                    } else {
                        ui.label("Enter self-employment income and click Calculate to see results.");
                    }
                });
            });

            ui.add_space(20.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("← Back to Main").clicked() {
                    app.current_screen = Screen::Main;
                    app.form.errors.clear();
                }

                if ui.button("Calculate SE Tax").clicked() {
                    app.calculate_se_only();
                }

                if app.results.se_tax.is_some() {
                    if ui.button("Continue to Main Estimate →").clicked() {
                        app.current_screen = Screen::Main;
                        app.form.errors.clear();
                        app.show_message(
                            "SE tax will be included in main estimate",
                            MessageType::Info,
                        );
                    }
                }
            });
        });
    }

    /// Create a grid for currency input alignment
    fn currency_grid(ui: &mut Ui, id: &str, add_contents: impl FnOnce(&mut Ui)) {
        egui::Grid::new(id)
            .num_columns(3)
            .spacing([10.0, 8.0])
            .min_col_width(0.0)
            .show(ui, add_contents);
    }

    /// Add a row to a currency grid with consistent layout
    fn grid_currency_row(ui: &mut Ui, label: &str, value: &mut String, required: bool) {
        // Column 1: Label (fixed width for alignment)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.set_min_width(200.0);
            if required {
                ui.label(egui::RichText::new(label).strong());
            } else {
                ui.label(label);
            }
        });

        // Column 2: Dollar sign
        ui.label("$");

        // Column 3: Input field (fixed width)
        ui.add(
            egui::TextEdit::singleline(value)
                .desired_width(120.0)
                .hint_text("0.00"),
        );

        ui.end_row();
    }
}
