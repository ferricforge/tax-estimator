use crate::screens::{EstimateListScreen, MainEstimateScreen, SelfEmploymentScreen};
use egui::Context;
use rust_decimal::Decimal;
use std::str::FromStr;
use tax_core::models::{NewTaxEstimate, TaxEstimate};

/// Which screen is currently active
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Main,
    SelfEmployment,
    LoadEstimate,
}

/// Form state for creating/editing an estimate
#[derive(Debug, Clone, Default)]
pub struct EstimateForm {
    pub tax_year: String,
    pub filing_status: FilingStatus,
    pub expected_agi: String,
    pub expected_deduction: String,
    pub expected_qbi_deduction: String,
    pub expected_amt: String,
    pub expected_credits: String,
    pub expected_other_taxes: String,
    pub expected_withholding: String,
    pub prior_year_tax: String,

    // SE fields
    pub se_income: String,
    pub expected_crp_payments: String,
    pub expected_wages: String,

    // Validation errors
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilingStatus {
    #[default]
    Single,
    MarriedFilingJointly,
    MarriedFilingSeparately,
    HeadOfHousehold,
    QualifyingWidow,
}

impl FilingStatus {
    pub fn all() -> &'static [FilingStatus] {
        &[
            FilingStatus::Single,
            FilingStatus::MarriedFilingJointly,
            FilingStatus::MarriedFilingSeparately,
            FilingStatus::HeadOfHousehold,
            FilingStatus::QualifyingWidow,
        ]
    }

    pub fn id(&self) -> i32 {
        match self {
            FilingStatus::Single => 1,
            FilingStatus::MarriedFilingJointly => 2,
            FilingStatus::MarriedFilingSeparately => 3,
            FilingStatus::HeadOfHousehold => 4,
            FilingStatus::QualifyingWidow => 5,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FilingStatus::Single => "Single",
            FilingStatus::MarriedFilingJointly => "Married Filing Jointly",
            FilingStatus::MarriedFilingSeparately => "Married Filing Separately",
            FilingStatus::HeadOfHousehold => "Head of Household",
            FilingStatus::QualifyingWidow => "Qualifying Widow(er)",
        }
    }
}

// Standalone parsing functions that collect errors into a Vec
fn parse_required<T: FromStr>(errors: &mut Vec<String>, field: &str, value: &str) -> Option<T> {
    if value.trim().is_empty() {
        errors.push(format!("{field} is required"));
        return None;
    }
    match value.trim().parse() {
        Ok(v) => Some(v),
        Err(_) => {
            errors.push(format!("{field} is invalid"));
            None
        }
    }
}

fn parse_decimal_required(errors: &mut Vec<String>, field: &str, value: &str) -> Option<Decimal> {
    if value.trim().is_empty() {
        errors.push(format!("{field} is required"));
        return None;
    }
    match Decimal::from_str(value.trim()) {
        Ok(v) => Some(v),
        Err(_) => {
            errors.push(format!("{field} must be a valid number"));
            None
        }
    }
}

fn parse_decimal_optional(value: &str) -> Option<Decimal> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Decimal::from_str(trimmed).ok()
}

impl EstimateForm {
    pub fn new() -> Self {
        Self {
            tax_year: "2025".to_string(),
            expected_deduction: "14600".to_string(),
            ..Default::default()
        }
    }

    /// Parse form into NewTaxEstimate, returning errors if invalid
    pub fn validate(&mut self) -> Result<NewTaxEstimate, ()> {
        let mut errors = Vec::new();

        let tax_year = parse_required(&mut errors, "Tax Year", &self.tax_year);
        let expected_agi = parse_decimal_required(&mut errors, "Expected AGI", &self.expected_agi);
        let expected_deduction =
            parse_decimal_required(&mut errors, "Expected Deduction", &self.expected_deduction);

        let expected_qbi_deduction = parse_decimal_optional(&self.expected_qbi_deduction);
        let expected_amt = parse_decimal_optional(&self.expected_amt);
        let expected_credits = parse_decimal_optional(&self.expected_credits);
        let expected_other_taxes = parse_decimal_optional(&self.expected_other_taxes);
        let expected_withholding = parse_decimal_optional(&self.expected_withholding);
        let prior_year_tax = parse_decimal_optional(&self.prior_year_tax);
        let se_income = parse_decimal_optional(&self.se_income);
        let expected_crp_payments = parse_decimal_optional(&self.expected_crp_payments);
        let expected_wages = parse_decimal_optional(&self.expected_wages);

        self.errors = errors;

        if !self.errors.is_empty() {
            return Err(());
        }

        Ok(NewTaxEstimate {
            tax_year: tax_year.unwrap(),
            filing_status_id: self.filing_status.id(),
            expected_agi: expected_agi.unwrap(),
            expected_deduction: expected_deduction.unwrap(),
            expected_qbi_deduction,
            expected_amt,
            expected_credits,
            expected_other_taxes,
            expected_withholding,
            prior_year_tax,
            se_income,
            expected_crp_payments,
            expected_wages,
        })
    }

    /// Validate only SE fields for SE tax calculation
    pub fn validate_se_only(&mut self) -> Result<SeInputs, ()> {
        let mut errors = Vec::new();

        let se_income = if self.se_income.trim().is_empty() {
            errors.push("Self-Employment Income is required".to_string());
            None
        } else {
            match Decimal::from_str(self.se_income.trim()) {
                Ok(v) => Some(v),
                Err(_) => {
                    errors.push("Self-Employment Income must be a valid number".to_string());
                    None
                }
            }
        };

        let crp_payments = parse_decimal_optional(&self.expected_crp_payments);
        let wages = parse_decimal_optional(&self.expected_wages);

        self.errors = errors;

        if !self.errors.is_empty() {
            return Err(());
        }

        Ok(SeInputs {
            se_income: se_income.unwrap(),
            crp_payments,
            wages,
        })
    }
}

/// Parsed SE inputs for calculation
#[derive(Debug, Clone)]
pub struct SeInputs {
    pub se_income: Decimal,
    pub crp_payments: Option<Decimal>,
    pub wages: Option<Decimal>,
}

/// Calculated results to display
#[derive(Debug, Clone, Default)]
pub struct CalculationResults {
    pub se_tax: Option<Decimal>,
    pub total_tax: Option<Decimal>,
    pub required_payment: Option<Decimal>,
    pub quarterly_payment: Option<Decimal>,
}

/// Main application state
pub struct TaxApp {
    pub current_screen: Screen,
    pub form: EstimateForm,
    pub results: CalculationResults,
    pub saved_estimates: Vec<TaxEstimate>,
    pub selected_estimate_id: Option<i64>,
    pub status_message: Option<(String, MessageType)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Info,
    Success,
    Error,
}

impl TaxApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            current_screen: Screen::Main,
            form: EstimateForm::new(),
            results: CalculationResults::default(),
            saved_estimates: Vec::new(),
            selected_estimate_id: None,
            status_message: None,
        }
    }

    pub fn show_message(&mut self, msg: impl Into<String>, msg_type: MessageType) {
        self.status_message = Some((msg.into(), msg_type));
    }

    pub fn clear_message(&mut self) {
        self.status_message = None;
    }

    /// Calculate only SE tax
    pub fn calculate_se_only(&mut self) {
        match self.form.validate_se_only() {
            Ok(inputs) => {
                // SE tax = net earnings * 92.35% * 15.3%
                let se_base = inputs.se_income * Decimal::from_str("0.9235").unwrap();
                let se_tax = se_base * Decimal::from_str("0.153").unwrap();

                self.results.se_tax = Some(se_tax);
                self.show_message("SE tax calculated", MessageType::Success);
            }
            Err(()) => {
                self.show_message("Please fix validation errors", MessageType::Error);
            }
        }
    }

    /// Full calculation
    pub fn calculate(&mut self) {
        match self.form.validate() {
            Ok(estimate) => {
                let se_tax = estimate.se_income.map(|se| {
                    let se_base = se * Decimal::from_str("0.9235").unwrap();
                    se_base * Decimal::from_str("0.153").unwrap()
                });

                self.results = CalculationResults {
                    se_tax,
                    total_tax: Some(Decimal::from(10000)),
                    required_payment: Some(Decimal::from(8000)),
                    quarterly_payment: Some(Decimal::from(2000)),
                };

                self.show_message("Calculation complete", MessageType::Success);
            }
            Err(()) => {
                self.show_message("Please fix validation errors", MessageType::Error);
            }
        }
    }
}

impl eframe::App for TaxApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Estimate").clicked() {
                        self.form = EstimateForm::new();
                        self.results = CalculationResults::default();
                        self.current_screen = Screen::Main;
                        ui.close_menu();
                    }
                    if ui.button("Load Estimate...").clicked() {
                        self.current_screen = Screen::LoadEstimate;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        egui::SidePanel::left("nav_panel")
            .resizable(false)
            .default_width(150.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Navigation");
                });
                ui.separator();

                if ui
                    .selectable_label(self.current_screen == Screen::Main, "📋 Main Estimate")
                    .clicked()
                {
                    self.current_screen = Screen::Main;
                }

                if ui
                    .selectable_label(
                        self.current_screen == Screen::SelfEmployment,
                        "💼 Self-Employment",
                    )
                    .clicked()
                {
                    self.current_screen = Screen::SelfEmployment;
                }

                if ui
                    .selectable_label(
                        self.current_screen == Screen::LoadEstimate,
                        "📂 Load Estimate",
                    )
                    .clicked()
                {
                    self.current_screen = Screen::LoadEstimate;
                }
            });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some((msg, msg_type)) = &self.status_message {
                    let color = match msg_type {
                        MessageType::Info => egui::Color32::GRAY,
                        MessageType::Success => egui::Color32::GREEN,
                        MessageType::Error => egui::Color32::RED,
                    };
                    ui.colored_label(color, msg);

                    if ui.small_button("✖").clicked() {
                        self.clear_message();
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.current_screen {
            Screen::Main => MainEstimateScreen::show(self, ui),
            Screen::SelfEmployment => SelfEmploymentScreen::show(self, ui),
            Screen::LoadEstimate => EstimateListScreen::show(self, ui),
        });
    }
}
