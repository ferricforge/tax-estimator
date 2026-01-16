# Data Model Evaluation

This document evaluates the implemented data model against the documented requirements for the Tax Estimator application.

## User Input Field Coverage

### SE Worksheet — All Fields Present

| Documented Requirement | Model Field | Status |
|------------------------|-------------|--------|
| Income subject to SE tax | `se_income` | ✓ |
| Expected CRP payments | `expected_crp_payments` | ✓ |
| Expected wages | `expected_wages` | ✓ |

### 1040-ES Worksheet — All Fields Present

| Documented Requirement | Model Field | Status |
|------------------------|-------------|--------|
| AGI estimate | `expected_agi` | ✓ |
| Deduction (Standard/Itemized) | `expected_deduction` | ✓ |
| QBI deduction | `expected_qbi_deduction` | ✓ |
| AMT estimate | `expected_amt` | ✓ |
| Credits estimate | `expected_credits` | ✓ |
| Other taxes | `expected_other_taxes` | ✓ |
| Prior year tax | `prior_year_tax` | ✓ |
| Withholding estimate | `expected_withholding` | ✓ |

## TaxYearConfig — Well Designed

The configurable multipliers and thresholds align with the original requirement to store rates rather than computed values:

| Field | Purpose | 2025 Seed Value |
|-------|---------|-----------------|
| `ss_wage_max` | SE Worksheet line 5-6 limit | $176,100 |
| `ss_tax_rate` | Combined SS rate (12.4%) | 0.124 |
| `medicare_tax_rate` | SE Worksheet line 4 (2.9%) | 0.029 |
| `se_tax_deductible_percentage` | SE Worksheet line 3 (92.35%) | 0.9235 |
| `se_deduction_factor` | SE deduction half (50%) | 0.50 |
| `required_payment_threshold` | 1040-ES line 14c minimum | $1,000 |
| `min_se_threshold` | Minimum SE income to trigger tax ($400) | $400 |

**Note**: The `min_se_threshold` field was added via migration `20260107000000_add_min_se_threshold.sql` and is actively used in the SE worksheet calculation to determine if SE tax applies.

## Migration SQL — Generally Sound

### Strengths

- Proper foreign key relationships between tables
- Appropriate decimal precision: `DECIMAL(12,2)` for currency, `DECIMAL(5,4)` for rates
- Complete seed data for all 5 filing statuses
- Tax brackets seeded for all schedules (X, Y-1, Y-2, Z, QSS)
- Nullable `max_income` correctly handles the top bracket (no upper limit)

### Standard Deduction Seed Values (Verified for 2025)

| Filing Status | Amount |
|---------------|--------|
| Single | $15,000 |
| Married Filing Jointly | $30,000 |
| Married Filing Separately | $15,000 |
| Head of Household | $22,500 |
| Qualifying Surviving Spouse | $30,000 |

## Identified Gaps

### High Priority

#### Additional Medicare Tax

The 0.9% Additional Medicare Tax on wages and self-employment income exceeding threshold amounts is not captured in the current model.

- **Thresholds**: $200,000 (Single/HOH), $250,000 (MFJ), $125,000 (MFS)
- **Impact**: Affects many self-employed filers with income over $200k
- **Recommendation**: Add `additional_medicare_threshold` and `additional_medicare_rate` to `TaxYearConfig`

### Medium Priority

#### Farm vs. Non-Farm SE Income

The SE Worksheet distinguishes between:
- Line 1a: Net farm profit (Schedule F)
- Line 1b: Net non-farm profit (Schedule C/SE)

The current model combines these into a single `se_income` field.

- **Impact**: Minor for most users; calculation is the same
- **Recommendation**: Consider splitting into `se_farm_income` and `se_nonfarm_income` for more accurate form representation

#### Safe Harbor 110% Rule

For taxpayers with prior year AGI exceeding $150,000 ($75,000 MFS), the safe harbor requirement is 110% of prior year tax rather than 100%.

- **Impact**: High-income users may underpay if using 100% safe harbor
- **Recommendation**: Consider storing `prior_year_agi` to determine which safe harbor percentage applies

### Lower Priority

#### Net Investment Income Tax (NIIT)

The 3.8% tax on net investment income for high earners (AGI over $200k single, $250k MFJ) is not modeled.

- **Impact**: Primarily affects users with significant investment income
- **Recommendation**: Add if target users include investors

#### Quarterly Payment Schedule

The model stores `calculated_required_payment` but does not track:
- Quarterly payment amounts
- Due dates (April 15, June 15, September 15, January 15)
- Payments already made

- **Recommendation**: Could add a `payment_schedule` table for tracking

#### Database Indexes

No indexes exist beyond primary keys. Queries on `(tax_year, filing_status_id)` could benefit from compound indexes.

- **Affected Tables**: `tax_brackets`, `standard_deductions`
- **Recommendation**: Add indexes for query performance at scale
- **Example**: `CREATE INDEX idx_tax_brackets_lookup ON tax_brackets(tax_year, filing_status_id);`

#### Input Validation

The model does not enforce business rules at the database level:

- **Negative values**: No CHECK constraints prevent negative amounts where they shouldn't be allowed (e.g., `expected_agi`, `expected_deduction`)
- **Required field combinations**: No validation that SE-related fields are provided together when `se_income` is present
- **Recommendation**: Add CHECK constraints or application-level validation to ensure data integrity

#### Timestamp Handling

The `tax_estimate` table uses SQLite's `TIMESTAMP` type with `CURRENT_TIMESTAMP` defaults, but the Rust model uses `chrono::DateTime<Utc>`:

- **Potential issue**: SQLite TIMESTAMP is stored as text/real, which may cause timezone conversion issues
- **Recommendation**: Consider using INTEGER for Unix timestamps or ensure consistent UTC handling in application code

## Summary

The core data model is well-aligned with the documented requirements. All user input fields from both the SE Worksheet and 1040-ES Worksheet are present, and the year-specific configuration captures the key multipliers and thresholds.

### What Works Well

- Complete coverage of documented user input fields
- Clean separation between configuration data and user calculations
- Flexible year-based configuration allows easy updates for new tax years
- Repository trait abstraction enables multiple database backends
- Proper use of `Option<T>` for optional fields in the Rust model
- Migration system supports schema evolution (e.g., `min_se_threshold` addition)
- Calculation logic is well-structured with separate worksheet modules

### For a Minimal Viable Implementation

The current model is sufficient to:

1. Calculate self-employment tax using the SE worksheet logic
2. Compute income tax using progressive brackets
3. Determine required estimated tax payments
4. Support all five filing statuses

### For Enhanced Accuracy

Consider adding support for:

1. **Additional Medicare Tax** (high priority for SE filers with income > $200k)
2. **Safe harbor 110% rule** (important for high-income users)
3. **Farm/non-farm income distinction** (nice-to-have for accuracy)

### For Production Readiness

Additional improvements to consider:

1. **Database indexes** on frequently queried columns (`tax_year`, `filing_status_id`)
2. **Input validation** via CHECK constraints or application-level validation
3. **Audit trail** for tracking changes to tax estimates (who/when/what changed)
4. **Soft deletes** for `tax_estimate` records (add `deleted_at` timestamp)
5. **Data retention policy** support (archive old estimates after N years)
