use crate::app::TaxApp;
use crate::widgets::currency_field;
use egui::Ui;

pub struct SelfEmploymentScreen;

impl SelfEmploymentScreen {
    pub fn show(app: &mut TaxApp, ui: &mut Ui) {
        ui.heading("Self-Employment Tax Worksheet");
        ui.separator();

        ui.label("Complete this section if you have self-employment income.");
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.group(|ui| {
                ui.heading("Self-Employment Income");

                currency_field(
                    ui,
                    "Net Self-Employment Income:",
                    &mut app.form.se_income,
                );

                ui.add_space(5.0);
                ui.label("Enter your expected net profit from Schedule C, Schedule F, or K-1.");
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.heading("Additional SE Information");

                currency_field(
                    ui,
                    "CRP Payments:",
                    &mut app.form.expected_crp_payments,
                );

                currency_field(
                    ui,
                    "Wages (if also employed):",
                    &mut app.form.expected_wages,
                );

                ui.add_space(5.0);
                ui.label("Wages affect the Social Security portion of SE tax calculation.");
            });

            ui.add_space(20.0);

            // SE Tax Preview
            ui.group(|ui| {
                ui.heading("SE Tax Preview");

                if let Some(se_tax) = app.results.se_tax {
                    egui::Grid::new("se_results")
                        .num_columns(2)
                        .spacing([20.0, 8.0])
                        .show(ui, |ui| {
                            ui.label("Estimated SE Tax:");
                            ui.strong(format!("${se_tax:.2}"));
                            ui.end_row();

                            // Deductible portion (50% of SE tax)
                            let deductible = se_tax / rust_decimal::Decimal::TWO;
                            ui.label("Deductible Portion (Line 6):");
                            ui.label(format!("${deductible:.2}"));
                            ui.end_row();
                        });
                } else {
                    ui.label("Enter self-employment income and calculate to see results.");
                }
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("← Back to Main").clicked() {
                    app.current_screen = crate::app::Screen::Main;
                }

                if ui.button("Calculate").clicked() {
                    app.calculate();
                }
            });
        });
    }
}
