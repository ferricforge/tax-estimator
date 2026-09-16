//! Reference data for the tax year the user is working in, cached as a gpui
//! global so every form reads the same values.

use gpui::{App, AsyncApp, BorrowAppContext, Global};
use rust_decimal::Decimal;

use crate::models::TaxYearData;
use crate::repository::TaxRepo;

/// The tax year the forms are working in, together with its configuration
/// once the fetch finishes.
#[derive(Clone, Debug, Default)]
pub struct ActiveTaxYear {
    year: Option<i32>,
    tax_year_data: Option<TaxYearData>,
}

impl Global for ActiveTaxYear {}

impl ActiveTaxYear {
    /// A year whose data has been requested but has not arrived.
    pub fn pending(year: i32) -> Self {
        Self {
            year: Some(year),
            tax_year_data: None,
        }
    }

    /// A year whose data is present.
    pub fn loaded(
        year: i32,
        tax_year_data: TaxYearData,
    ) -> Self {
        Self {
            year: Some(year),
            tax_year_data: Some(tax_year_data),
        }
    }

    /// The selected year, whether or not its data has arrived.
    pub fn year(&self) -> Option<i32> {
        self.year
    }

    /// The loaded data, whichever year it belongs to.
    pub fn data(&self) -> Option<&TaxYearData> {
        self.tax_year_data.as_ref()
    }

    /// The loaded data when it is for `year`; `None` when nothing is loaded
    /// or the loaded data is for a different year.
    pub fn data_for(
        &self,
        year: i32,
    ) -> Option<&TaxYearData> {
        self.tax_year_data
            .as_ref()
            .filter(|_| self.year == Some(year))
    }

    /// Whether the data for `year` is already in hand.
    pub fn is_loaded_for(
        &self,
        year: i32,
    ) -> bool {
        self.data_for(year).is_some()
    }

    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn ss_wage_max(cx: &App) -> Option<Decimal> {
        cx.try_global::<Self>()
            .and_then(Self::data)
            .map(|tax_year_data| tax_year_data.config.ss_wage_max)
    }

    /// Discards the cached year. Used when the active database changes, since
    /// the new database may carry different reference data.
    pub fn reset(cx: &mut App) {
        cx.set_global(Self::default());
    }

    /// Starts a fetch for `year`. No-op when the data is already loaded.
    pub fn load(
        year: i32,
        cx: &mut App,
    ) {
        if cx
            .try_global::<Self>()
            .is_some_and(|active| active.is_loaded_for(year))
        {
            tracing::info!("Already have tax year");
            return;
        }

        let Some(repo) = TaxRepo::try_get(cx) else {
            tracing::warn!("TaxRepo not initialised; cannot load tax year {year}");
            return;
        };

        tracing::info!("Setting global year {year}");
        cx.set_global(Self::pending(year));

        cx.spawn(async move |async_cx: &mut AsyncApp| {
            // `&*repo` dereferences the handle to `&dyn TaxRepository`.
            match TaxYearData::load(&*repo, year).await {
                Ok(tax_year_data) => {
                    let _ = async_cx.update(|cx| {
                        // update_global notifies observe_global subscribers
                        cx.update_global::<Self, _>(|active, _| {
                            active.year = Some(year);
                            active.tax_year_data = Some(tax_year_data);
                            tracing::trace!("Tax year load: {:#?}", active);
                        });
                    });
                }
                Err(e) => {
                    tracing::warn!(%e, year, "Failed to load TaxYearConfig");
                }
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use tax_core::TaxYearConfig;

    use super::*;

    fn tax_year_data(year: i32) -> TaxYearData {
        TaxYearData {
            config: TaxYearConfig {
                tax_year: year,
                ss_wage_max: Decimal::ZERO,
                ss_tax_rate: Decimal::ZERO,
                medicare_tax_rate: Decimal::ZERO,
                se_tax_deduct_pcnt: Decimal::ZERO,
                se_deduction_factor: Decimal::ZERO,
                req_pmnt_threshold: Decimal::ZERO,
                min_se_threshold: Decimal::ZERO,
            },
            statuses: Vec::new(),
        }
    }

    #[test]
    fn data_for_is_none_when_nothing_is_loaded() {
        assert!(ActiveTaxYear::default().data_for(2025).is_none());
    }

    #[test]
    fn data_for_requires_matching_year() {
        let active = ActiveTaxYear::loaded(2024, tax_year_data(2024));

        assert!(active.data_for(2025).is_none());
        assert!(active.data_for(2024).is_some());
    }

    #[test]
    fn pending_reports_the_year_without_data() {
        let active = ActiveTaxYear::pending(2025);

        assert_eq!(active.year(), Some(2025));
        assert!(active.data().is_none());
        assert!(!active.is_loaded_for(2025));
    }

    #[test]
    fn is_loaded_for_agrees_with_data_for() {
        let active = ActiveTaxYear::loaded(2025, tax_year_data(2025));

        assert!(active.is_loaded_for(2025));
        assert!(!active.is_loaded_for(2024));
    }
}
