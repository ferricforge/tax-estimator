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

impl EstimateForm {
    pub fn new() -> Self {
        Self {
            tax_year: "2025".to_string(),
            expected_deduction: "14600".to_string(), // 2024 standard deduction single
            ..Default::default()
        }
    }

    /// Parse form into NewTaxEstimate, returning errors if invalid
    pub fn validate(&mut self) -> Result<NewTaxEstimate, ()> {
        self.errors.clear();

        let tax_year = self.parse_required("Tax Year", &self.tax_year);
        let expected_agi = self.parse_decimal_required("Expected AGI", &self.expected_agi);
        let expected_deduction =
            self.parse_decimal_required("Expected Deduction", &self.expected_deduction);

        let expected_qbi_deduction = self.parse_decimal_optional(&self.expected_qbi_deduction);
        let expected_amt = self.parse_decimal_optional(&self.expected_amt);
        let expected_credits = self.parse_decimal_optional(&self.expected_credits);
        let expected_other_taxes = self.parse_decimal_optional(&self.expected_other_taxes);
        let expected_withholding = self.parse_decimal_optional(&self.expected_withholding);
        let prior_year_tax = self.parse_decimal_optional(&self.prior_year_tax);
        let se_income = self.parse_decimal_optional(&self.se_income);
        let expected_crp_payments = self.parse_decimal_optional(&self.expected_crp_payments);
        let expected_wages = self.parse_decimal_optional(&self.expected_wages);

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

    fn parse_required<T: FromStr>(&mut self, field: &str, value: &str) -> Option<T> {
        if value.trim().is_empty() {
            self.errors.push(format!("{field} is required"));
            return None;
        }
        match value.trim().parse() {
            Ok(v) => Some(v),
            Err(_) => {
                self.errors.push(format!("{field} is invalid"));
                None
            }
        }
    }

    fn parse_decimal_required(&mut self, field: &str, value: &str) -> Option<Decimal> {
        if value.trim().is_empty() {
            self.errors.push(format!("{field} is required"));
            return None;
        }
        match Decimal::from_str(value.trim()) {
            Ok(v) => Some(v),
            Err(_) => {
                self.errors.push(format!("{field} must be a valid number"));
                None
            }
        }
    }

    fn parse_decimal_optional(&mut self, value: &str) -> Option<Decimal> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        match Decimal::from_str(trimmed) {
            Ok(v) => Some(v),
            Err(_) => None, // Silently ignore invalid optional fields
        }
    }
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

    /// Placeholder for actual calculation logic
    pub fn calculate(&mut self) {
        match self.form.validate() {
            Ok(estimate) => {
                // TODO: Call your actual tax calculation logic from tax-core
                // For now, just show placeholder results
                let se_tax = estimate.se_income.map(|se| se * Decimal::from_str("0.153").unwrap());

                self.results = CalculationResults {
                    se_tax,
                    total_tax: Some(Decimal::from(10000)), // Placeholder
                    required_payment: Some(Decimal::from(8000)), // Placeholder
                    quarterly_payment: Some(Decimal::from(2000)), // Placeholder
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
        // Top menu bar
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

        // Navigation sidebar
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

        // Status bar at bottom
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

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| match self.current_screen {
            Screen::Main => MainEstimateScreen::show(self, ui),
            Screen::SelfEmployment => SelfEmploymentScreen::show(self, ui),
            Screen::LoadEstimate => EstimateListScreen::show(self, ui),
        });
    }
}
