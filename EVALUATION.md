# Outstanding Data Model Items

This document tracks remaining data model, calculation, and persistence work for
the Tax Estimator application.

## High Priority

### Additional Medicare Tax

Add support for the 0.9% Additional Medicare Tax on wages, railroad retirement
compensation, and self-employment income above filing-status thresholds.

- **Thresholds**: $200,000 for Single and Head of Household, $250,000 for
  Married Filing Jointly and Qualifying Surviving Spouse, and $125,000 for
  Married Filing Separately.
- **Configuration**: Add tax-year-specific values for
  `additional_medicare_rate` and the applicable threshold by filing status.
- **Calculation**: Apply the tax to qualifying wages and self-employment income
  above the threshold, with tests for mixed wage and self-employment cases.
- **Persistence/UI**: Store or display the calculated amount separately if users
  need worksheet-level transparency.

### Safe Harbor 110% Rule

Add automatic support for the high-income safe harbor rule.

- **Input**: Add `prior_year_agi` or an explicit safe-harbor multiplier override.
- **Rule**: Use 110% of prior-year tax when prior-year AGI exceeds $150,000, or
  $75,000 for Married Filing Separately; otherwise use 100%.
- **Calculation**: Apply the adjusted prior-year tax amount before comparing it
  with the current-year required payment amount.
- **Tests**: Cover both 100% and 110% safe-harbor cases, including the point
  where the prior-year tax amount controls the required annual payment.

## Medium Priority

### Farm vs. Non-Farm Self-Employment Income

Track Schedule SE farm and non-farm income as separate worksheet inputs.

- **Fields**: Add fields such as `se_farm_income` and `se_nonfarm_income`, or
  align names directly with Schedule SE line 1a and line 2.
- **CRP payments**: Keep Conservation Reserve Program payments as a separate
  line 1b input.
- **Surfaces**: Update persistence, UI forms, CSV import, and worksheet tests.

### Net Investment Income Tax

Add explicit Net Investment Income Tax support for taxpayers with investment
income over filing-status thresholds.

- **Inputs**: Add net investment income and threshold data by filing status.
- **Rule**: Apply the 3.8% NIIT calculation for qualifying high-income users.
- **Integration**: Feed the calculated NIIT into the estimated tax worksheet's
  other-taxes path, or store it separately when shown in results.

### Farmer/Fisher Estimated Tax Path

Expose the farmer/fisher estimated tax rule as a user-controlled input.

- **Input**: Add a persisted `is_farmer_or_fisher` field if this status should
  be user-selectable.
- **Calculation**: Use the two-thirds current-year factor for qualifying users.
- **Surfaces**: Include the flag in UI, CSV import/export, and end-to-end tests.

### Saved Estimate Loading

Add a UI flow for selecting a persisted estimate and restoring it into the
current worksheet.

- **Selection**: List saved estimates, optionally filtered by tax year, with
  enough context to distinguish filing status, update time, and key amounts.
- **Hydration**: Populate the main 1040-ES form, SE worksheet fields, active tax
  year state, and filing status from the selected `TaxEstimate`.
- **Results**: Display saved computed values when present, and make recalculation
  available after the estimate is loaded.
- **Tests**: Cover loading an estimate with full optional input values, no
  optional values, and persisted computed results.

## Lower Priority

### Quarterly Payment Schedule

Add support for tracking estimated payments across payment periods.

- **Schedule**: Store quarterly due dates and calculated payment amounts.
- **Payments**: Track payments already made, payment dates, and remaining
  balance by period.
- **Reporting**: Show annual and per-quarter totals for planning and review.

### Database-Level Validation

Add database constraints for core business rules that should be enforced at the
storage layer.

- **Nonnegative values**: Add `CHECK` constraints for amounts that should not be
  negative, such as AGI, deductions, credits, withholding, and prior-year tax.
- **Rate bounds**: Constrain tax rates and multipliers to valid ranges.
- **Computed values**: Enforce all-or-none population for calculated result
  columns such as SE tax, total tax, and required payment.

### Timestamp Storage

Normalize timestamp storage so persisted values have unambiguous UTC semantics.

- **Format**: Consider storing Unix timestamps as integers, or ISO 8601 strings
  with explicit UTC offsets.
- **Conversion**: Ensure reads and writes round-trip cleanly through
  `chrono::DateTime<Utc>`.
- **Tests**: Add persistence tests around timestamp parsing and ordering.

### Query Performance Review

Review query plans for common lookup and listing paths, then add indexes where
they provide measurable value.

- **Estimate listing**: Consider an index on `tax_estimate(tax_year, updated_at)`
  if filtered estimate lists become large.
- **Lookup paths**: Verify that existing keys cover tax bracket and standard
  deduction lookups efficiently.
- **Validation**: Add indexes only after confirming the expected query plans.

### Audit Trail, Soft Deletes, and Retention

Add production data-lifecycle features if estimates need long-term management.

- **Audit trail**: Track when estimates change and, if user accounts are added,
  who changed them.
- **Soft deletes**: Add a `deleted_at` timestamp for recoverable deletes.
- **Retention**: Define archive or purge behavior for old estimates.
