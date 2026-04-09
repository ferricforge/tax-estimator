# Tax Estimate Persistence Plan

## Summary

`call_calculate_tax_estimate()` in `tax-ui/src/components/estimate_form.rs` already does the hard parts of assembling `TaxEstimateInput` and calculating `EstimatedTaxWorksheetResult`, but it does not persist anything yet. The missing work is to bridge the synchronous click handler to the async repository layer and write both the input payload and computed values to `tax_estimate`.

The implementation should keep the current canonical model shape:

- `TaxEstimateInput` for user-entered values
- `TaxEstimateComputed` for stored calculated values
- `TaxEstimate` for the full persisted row

The persistence route should stay two-step for now:

1. `create_estimate(input)` to insert the canonical input payload
2. `update_estimate(&tax_estimate)` to attach computed values

## Current Route

### Input assembly

`EstimateForm::to_input()` already builds a complete `TaxEstimateInput` from:

- form fields for:
  - `tax_year`
  - `filing_status`
  - `expected_agi`
  - `expected_deduction`
  - `expected_qbi_deduction`
  - `expected_amt`
  - `expected_credits`
  - `expected_other_taxes`
  - `expected_withholding`
  - `prior_year_tax`
- `SeWorksheetModel` for:
  - `se_income`
  - `expected_crp_payments`
  - `expected_wages`

`TaxEstimateInput::validate_for_submit()` already validates the assembled model before calculation or persistence.

### Calculation assembly

`call_calculate_tax_estimate()` already:

- reads `SeWorksheetModel`
- loads `ActiveTaxYear`
- finds the selected `FilingStatusData`
- builds `EstimatedTaxWorksheetContext`
- converts `TaxEstimateInput` into `EstimatedTaxWorksheetInput`
- calculates `EstimatedTaxWorksheetResult`

The context values currently come from:

- `self_employment_tax = se_model.line_10_total_se_tax.unwrap_or_default()`
- `refundable_credits = Decimal::ZERO`
- `is_farmer_or_fisher = false`
- `required_payment_threshold = config.req_pmnt_threshold`

### Repository path

The repository API already supports the needed write flow:

- `TaxRepository::create_estimate(estimate: TaxEstimateInput) -> TaxEstimate`
- `TaxRepository::update_estimate(estimate: &TaxEstimate) -> Result<(), RepositoryError>`

The SQLite implementation persists:

- all input fields in `create_estimate()`
- all computed fields in `update_estimate()`

## Implementation Changes

### 1. Persist from `call_calculate_tax_estimate()`

After the estimated-tax worksheet succeeds, replace the final log-only behavior with an async persistence flow using the global `TaxRepo`.

The handler should:

1. keep `form_input`
2. keep `se_model`
3. keep `result`
4. fetch the global repository handle with `TaxRepo::get(cx)` or `TaxRepo::try_get(cx)`
5. spawn an async task from the UI context
6. call `create_estimate(form_input.clone())`
7. clone the returned `TaxEstimate`
8. assign `computed = Some(TaxEstimateComputed { ... })`
9. call `update_estimate(&updated)`
10. log success and, if desired, trigger later UI state updates

### 2. Computed value mapping

The computed payload should be assembled exactly as:

- `se_tax = se_model.line_10_total_se_tax.unwrap_or_default()`
- `total_tax = result.total_estimated_tax`
- `required_payment = result.required_annual_payment`

This becomes:

```rust
TaxEstimateComputed {
    se_tax,
    total_tax,
    required_payment,
}
```

Do not persist `EstimatedTaxWorksheetResult` directly. Only persist the `TaxEstimateComputed` subset already represented in the domain model.

### 3. Async/UI boundary

Use the same async pattern already present elsewhere in `tax-ui`:

- capture the values needed by the async task
- call repository methods inside `cx.spawn(async move |async_cx| { ... })`
- detach the task

The click handler should remain responsible for:

- form validation
- worksheet calculation
- immediate calculation failure handling

The async task should be responsible for:

- repository writes
- persistence error logging
- showing an error dialog if repository writes fail

### 4. Error handling

Handle these failure points explicitly:

- no active repository global
- repository `create_estimate()` failure
- repository `update_estimate()` failure

Expected behavior:

- log the error with context
- show a dialog explaining that calculation succeeded or was attempted but persistence failed
- do not silently swallow persistence failures

### 5. Logging and success state

On success, log:

- the created estimate id
- the input summary
- the computed summary

If later UI feedback is added, the persisted `TaxEstimate` returned from create/update should become the source of truth for any saved-state display.

## Suggested Flow

```rust
let repo = TaxRepo::get(cx);
let input = form_input.clone();
let se_tax = se_model.line_10_total_se_tax.unwrap_or_default();
let total_tax = result.total_estimated_tax;
let required_payment = result.required_annual_payment;

cx.spawn(async move |async_cx| {
    let created = repo.tax_repository().create_estimate(input.clone()).await?;

    let mut updated = created.clone();
    updated.computed = Some(TaxEstimateComputed {
        se_tax,
        total_tax,
        required_payment,
    });

    repo.tax_repository().update_estimate(&updated).await?;

    let _ = async_cx.update(|_cx| {
        tracing::info!(estimate_id = updated.id, "Tax estimate persisted");
    });

    Ok::<(), RepositoryError>(())
})
.detach();
```

The exact UI update inside `async_cx.update()` can change, but the write order should not.

## Test Plan

Update or extend the existing integration coverage so it proves the full persistence route from assembled input through the repository.

Required scenarios:

- form input assembles a valid `TaxEstimateInput`
- estimated-tax worksheet produces the persisted computed values
- `create_estimate()` stores the input payload
- `update_estimate()` stores:
  - `se_tax`
  - `total_tax`
  - `required_payment`
- `get_estimate()` returns the same canonical input and computed values

Failure scenarios:

- repository unavailable from the UI global state
- create succeeds but update fails
- worksheet calculation fails before persistence and no row is written

The existing integration test in `tax-ui/tests/estimate_calculation_integration.rs` is the best template for expected data flow and should be kept aligned with the UI handler behavior.

## Assumptions

- The current two-step repository API is intentionally preserved.
- The database schema does not need to change for this work.
- Refundable credits remain `Decimal::ZERO` until separate UI support exists.
- Farmer/fisher behavior remains `false` until separate UI support exists.
- The plan covers persistence wiring only, not a saved-estimate list or edit workflow.
