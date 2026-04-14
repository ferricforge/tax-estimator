use std::collections::BTreeMap;
use std::sync::OnceLock;

use gpui::SharedString;
use serde::Deserialize;

const EMBEDDED_CATALOG: &str = include_str!("../resources/field_instructions.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldHelp {
    pub label: SharedString,
    pub paragraphs: Vec<SharedString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiInstructionField {
    ExpectedAgi,
    ExpectedDeduction,
    ExpectedQbiDeduction,
    ExpectedAmt,
    ExpectedCredits,
    ExpectedOtherTaxes,
    ExpectedWithholding,
    PriorYearTax,
    SeIncome,
    CrpPayments,
    SeLine2,
    SeLine3,
    SeLine4,
    SeLine5,
    ExpectedWages,
    SeLine7,
    SeLine8,
    SeLine9,
    SeLine10,
    SeLine11,
}

impl UiInstructionField {
    fn specs(self) -> Vec<FieldSpec> {
        match self {
            Self::ExpectedAgi => vec![FieldSpec::new("1040-es", "expected_agi")],
            Self::ExpectedDeduction => vec![FieldSpec::new("1040-es", "expected_deduction")],
            Self::ExpectedQbiDeduction => vec![
                FieldSpec::new("1040-es", "expected_qbi_deduction"),
                FieldSpec::new("8995", "expected_qbi_deduction"),
            ],
            Self::ExpectedAmt => vec![FieldSpec::new("1040-es", "expected_amt")],
            Self::ExpectedCredits => vec![FieldSpec::new("1040-es", "expected_credits")],
            Self::ExpectedOtherTaxes => vec![FieldSpec::new("1040-es", "expected_other_taxes")],
            Self::ExpectedWithholding => vec![FieldSpec::new("1040-es", "expected_withholding")],
            Self::PriorYearTax => vec![FieldSpec::new("1040-es", "prior_year_tax")],
            Self::SeIncome => vec![FieldSpec::new("se-worksheet", "se_income")],
            Self::CrpPayments => vec![FieldSpec::new("se-worksheet", "crp_payments")],
            Self::SeLine2 => vec![FieldSpec::new("se-worksheet", "line_2")],
            Self::SeLine3 => vec![FieldSpec::new("se-worksheet", "line_3")],
            Self::SeLine4 => vec![FieldSpec::new("se-worksheet", "line_4")],
            Self::SeLine5 => vec![FieldSpec::new("se-worksheet", "line_5")],
            Self::ExpectedWages => vec![FieldSpec::new("se-worksheet", "expected_wages")],
            Self::SeLine7 => vec![FieldSpec::new("se-worksheet", "line_7")],
            Self::SeLine8 => vec![FieldSpec::new("se-worksheet", "line_8")],
            Self::SeLine9 => vec![FieldSpec::new("se-worksheet", "line_9")],
            Self::SeLine10 => vec![FieldSpec::new("se-worksheet", "line_10")],
            Self::SeLine11 => vec![FieldSpec::new("se-worksheet", "line_11")],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FieldSpec {
    form_id: &'static str,
    field_key: &'static str,
}

impl FieldSpec {
    const fn new(
        form_id: &'static str,
        field_key: &'static str,
    ) -> Self {
        Self { form_id, field_key }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CatalogFile {
    forms: Vec<FormFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct FormFile {
    id: String,
    title: String,
    years: Vec<FormYearFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct FormYearFile {
    year: i32,
    fields: Vec<FormFieldFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct FormFieldFile {
    key: String,
    label: String,
    summary: String,
    detail: Option<String>,
    sources: Vec<FieldSourceFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct FieldSourceFile {
    file: String,
    page: u16,
    section: String,
}

#[derive(Debug, Clone)]
struct InstructionCatalog {
    forms: BTreeMap<String, FormCatalog>,
}

#[derive(Debug, Clone)]
struct FormCatalog {
    title: String,
    years: BTreeMap<i32, BTreeMap<String, FormFieldFile>>,
}

#[derive(Debug, Clone)]
struct ResolvedFieldPart {
    label: String,
    summary: String,
    detail: Option<String>,
    source_summary: String,
}

impl InstructionCatalog {
    fn load_embedded() -> Self {
        match toml::from_str::<CatalogFile>(EMBEDDED_CATALOG) {
            Ok(file) => Self::from_file(file),
            Err(error) => {
                tracing::error!(error = ?error, "Failed to parse embedded field instruction catalog");
                Self {
                    forms: BTreeMap::new(),
                }
            }
        }
    }

    fn from_file(file: CatalogFile) -> Self {
        let forms = file
            .forms
            .into_iter()
            .map(|form| {
                let years = form
                    .years
                    .into_iter()
                    .map(|year| {
                        let fields = year
                            .fields
                            .into_iter()
                            .map(|field| (field.key.clone(), field))
                            .collect();
                        (year.year, fields)
                    })
                    .collect();
                (
                    form.id,
                    FormCatalog {
                        title: form.title,
                        years,
                    },
                )
            })
            .collect();

        Self { forms }
    }

    fn resolve_part(
        &self,
        spec: FieldSpec,
        selected_year: Option<i32>,
    ) -> Option<ResolvedFieldPart> {
        let form = self.forms.get(spec.form_id)?;
        let resolved_year = selected_year
            .filter(|year| form.years.contains_key(year))
            .or_else(|| form.years.keys().next_back().copied())?;

        let field = form.years.get(&resolved_year)?.get(spec.field_key)?;
        let sources = field
            .sources
            .iter()
            .map(|source| {
                format!(
                    "{} ({}, p. {}, {})",
                    form.title, source.file, source.page, source.section
                )
            })
            .collect::<Vec<_>>()
            .join("; ");

        Some(ResolvedFieldPart {
            label: field.label.clone(),
            summary: field.summary.clone(),
            detail: field.detail.clone(),
            source_summary: sources,
        })
    }
}

static CATALOG: OnceLock<InstructionCatalog> = OnceLock::new();

pub(crate) fn help_for_field(
    field: UiInstructionField,
    selected_year: Option<i32>,
) -> Option<FieldHelp> {
    let catalog = CATALOG.get_or_init(InstructionCatalog::load_embedded);
    let mut label: Option<String> = None;
    let mut paragraphs = Vec::new();
    let mut sources = Vec::new();

    for spec in field.specs() {
        let part = catalog.resolve_part(spec, selected_year)?;
        if label.is_none() {
            label = Some(part.label.clone());
        }
        push_unique(&mut paragraphs, part.summary);
        if let Some(detail) = part.detail {
            push_unique(&mut paragraphs, detail);
        }
        push_unique(&mut sources, part.source_summary);
    }

    if sources.is_empty() {
        return None;
    }

    paragraphs.push(format!("Source: {}", sources.join(" | ")));

    Some(FieldHelp {
        label: SharedString::from(label?),
        paragraphs: paragraphs.into_iter().map(SharedString::from).collect(),
    })
}

fn push_unique(
    values: &mut Vec<String>,
    candidate: String,
) {
    if !values.iter().any(|value| value == &candidate) {
        values.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::{UiInstructionField, help_for_field};

    #[test]
    fn uses_exact_year_when_available() {
        let help = help_for_field(UiInstructionField::ExpectedAgi, Some(2026))
            .expect("2026 AGI help should resolve");

        assert!(help.paragraphs[0].contains("2026"));
    }

    #[test]
    fn falls_back_to_latest_available_year() {
        let help = help_for_field(UiInstructionField::ExpectedAgi, Some(2030))
            .expect("fallback AGI help should resolve");

        assert!(help.paragraphs[0].contains("2026"));
    }

    #[test]
    fn merges_qbi_instructions_from_multiple_forms() {
        let help = help_for_field(UiInstructionField::ExpectedQbiDeduction, Some(2026))
            .expect("QBI help should resolve");

        assert_eq!(help.label, "QBI deduction");
        assert!(
            help.paragraphs
                .iter()
                .any(|p| p.contains("qualified business income deduction"))
        );
        assert!(help.paragraphs.iter().any(|p| p.contains("Form 8995")));
    }
}
