use crate::screens::{EstimateListScreen, MainEstimateScreen, SelfEmploymentScreen};
use egui::Context;
use rust_decimal::Decimal;
use std::str::FromStr;
use tax_core::calculations::{SeWorksheet, SeWorksheetConfig};
use tax_core::models::{NewTaxEstimate, TaxEstimate};

/// Which screen is currently active
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Screen {
    #[default]
    Main,
    SelfEmployment,
    LoadEstimate,
}

/// Returns available tax years for selection
/// TODO: Replace with database retrieval
pub fn get_available_tax_years() -> Vec<i32> {
    vec![2026, 2025, 2024, 2023]
}

/// Form state for creating/editing an estimate
#[derive(Debug, Clone)]
pub struct EstimateForm {
    pub tax_year: i32,
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

impl Default for EstimateForm {
    fn default() -> Self {
        Self {
            tax_year: 2025,
            filing_status: FilingStatus::default(),
            expected_agi: String::new(),
            expected_deduction: "14600".to_string(),
            expected_qbi_deduction: String::new(),
            expected_amt: String::new(),
            expected_credits: String::new(),
            expected_other_taxes: String::new(),
            expected_withholding: String::new(),
            prior_year_tax: String::new(),
            se_income: String::new(),
            expected_crp_payments: String::new(),
            expected_wages: String::new(),
            errors: Vec::new(),
        }
    }
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
        Self::default()
    }

    /// Parse form into NewTaxEstimate, returning errors if invalid
    pub fn validate(&mut self) -> Result<NewTaxEstimate, ()> {
        let mut errors = Vec::new();

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
            tax_year: self.tax_year,
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

/// Returns SE worksheet configuration for a given tax year
/// TODO: Replace with database retrieval
pub fn get_se_config(tax_year: i32) -> SeWorksheetConfig {
    match tax_year {
        2026 => SeWorksheetConfig {
            // Projected values - update when IRS announces
            ss_wage_max: Decimal::from_str("180000.00").unwrap(),
            ss_tax_rate: Decimal::from_str("0.124").unwrap(),
            medicare_tax_rate: Decimal::from_str("0.029").unwrap(),
            net_earnings_factor: Decimal::from_str("0.9235").unwrap(),
            deduction_factor: Decimal::from_str("0.50").unwrap(),
            min_se_threshold: Decimal::from_str("400.00").unwrap(),
        },
        2025 => SeWorksheetConfig {
            ss_wage_max: Decimal::from_str("176100.00").unwrap(),
            ss_tax_rate: Decimal::from_str("0.124").unwrap(),
            medicare_tax_rate: Decimal::from_str("0.029").unwrap(),
            net_earnings_factor: Decimal::from_str("0.9235").unwrap(),
            deduction_factor: Decimal::from_str("0.50").unwrap(),
            min_se_threshold: Decimal::from_str("400.00").unwrap(),
        },
        2024 => SeWorksheetConfig {
            ss_wage_max: Decimal::from_str("168600.00").unwrap(),
            ss_tax_rate: Decimal::from_str("0.124").unwrap(),
            medicare_tax_rate: Decimal::from_str("0.029").unwrap(),
            net_earnings_factor: Decimal::from_str("0.9235").unwrap(),
            deduction_factor: Decimal::from_str("0.50").unwrap(),
            min_se_threshold: Decimal::from_str("400.00").unwrap(),
        },
        // 2023 and earlier / default
        _ => SeWorksheetConfig {
            ss_wage_max: Decimal::from_str("160200.00").unwrap(),
            ss_tax_rate: Decimal::from_str("0.124").unwrap(),
            medicare_tax_rate: Decimal::from_str("0.029").unwrap(),
            net_earnings_factor: Decimal::from_str("0.9235").unwrap(),
            deduction_factor: Decimal::from_str("0.50").unwrap(),
            min_se_threshold: Decimal::from_str("400.00").unwrap(),
        },
    }
}

/// Parsed SE inputs for calculation
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used when full SE calculation is implemented
pub struct SeInputs {
    pub se_income: Decimal,
    pub crp_payments: Option<Decimal>,
    pub wages: Option<Decimal>,
}

/// Calculated results to display
#[derive(Debug, Clone, Default)]
pub struct CalculationResults {
    pub se_tax: Option<Decimal>,
    pub se_tax_deduction: Option<Decimal>,
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

    /// Calculate only SE tax using tax-core SeWorksheet
    pub fn calculate_se_only(&mut self) {
        match self.form.validate_se_only() {
            Ok(inputs) => {
                let config = get_se_config(self.form.tax_year);
                let worksheet = SeWorksheet::new(config);

                let crp_payments = inputs.crp_payments.unwrap_or(Decimal::ZERO);
                let wages = inputs.wages.unwrap_or(Decimal::ZERO);

                match worksheet.calculate(inputs.se_income, crp_payments, wages) {
                    Ok(result) => {
                        self.results.se_tax = Some(result.self_employment_tax);
                        self.results.se_tax_deduction = Some(result.se_tax_deduction);
                        self.show_message(
                            format!(
                                "SE tax calculated: ${:.2}",
                                result.self_employment_tax
                            ),
                            MessageType::Success,
                        );
                    }
                    Err(e) => {
                        self.show_message(format!("SE calculation error: {e}"), MessageType::Error);
                    }
                }
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
                // Calculate SE tax if SE income is present
                let (se_tax, se_tax_deduction) = if let Some(se_income) = estimate.se_income {
                    let config = get_se_config(self.form.tax_year);
                    let worksheet = SeWorksheet::new(config);

                    let crp_payments = estimate.expected_crp_payments.unwrap_or(Decimal::ZERO);
                    let wages = estimate.expected_wages.unwrap_or(Decimal::ZERO);

                    match worksheet.calculate(se_income, crp_payments, wages) {
                        Ok(result) => {
                            (Some(result.self_employment_tax), Some(result.se_tax_deduction))
                        }
                        Err(e) => {
                            self.show_message(
                                format!("SE calculation error: {e}"),
                                MessageType::Error,
                            );
                            return;
                        }
                    }
                } else {
                    (None, None)
                };

                // TODO: Call EstimatedTaxWorksheet for full calculation
                // For now, use placeholder values
                self.results = CalculationResults {
                    se_tax,
                    se_tax_deduction,
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
