# Tax Estimator

A Rust application for calculating estimated federal income taxes based on IRS Form 1040-ES. Designed for U.S. residents of the 50 states.

## Overview

This workspace provides a modular architecture for estimating quarterly tax payments, supporting:

- Self-employment tax calculations (SE Worksheet)
- Progressive income tax computation using IRS tax brackets
- Multiple filing statuses (Single, MFJ, MFS, HOH, QSS)
- Year-specific tax parameters stored in a database

## Project Structure

```
tax-estimator/
├── tax-core/          # Core domain models and repository traits
├── tax-db-sqlite/     # SQLite database implementation
└── tax-ui/            # User interface (in development)
```

### Crates

| Crate | Description |
|-------|-------------|
| `tax-core` | Domain models (`TaxYearConfig`, `TaxBracket`, `FilingStatus`, etc.) and the `TaxRepository` trait for data access abstraction |
| `tax-db-sqlite` | SQLite implementation of `TaxRepository` with migrations and seed data for tax year 2025 |
| `tax-ui` | Terminal-based user interface (placeholder) |

## Data Model

### Tax Year Configuration (`TaxYearConfig`)

Stores year-specific constants:

| Field | Description | 2025 Value |
|-------|-------------|------------|
| `ss_wage_max` | Social Security wage base limit | $176,100 |
| `ss_tax_rate` | Combined SS tax rate for SE | 12.4% |
| `medicare_tax_rate` | Combined Medicare tax rate for SE | 2.9% |
| `se_tax_deductible_percentage` | Net earnings factor (line 3 of SE worksheet) | 92.35% |
| `se_deduction_factor` | Deductible portion of SE tax | 50% |
| `required_payment_threshold` | Minimum estimated tax due to require payments | $1,000 |

### Tax Brackets (`TaxBracket`)

Progressive tax rate schedule by filing status, including:
- Income thresholds (`min_income`, `max_income`)
- Marginal rate (`tax_rate`)
- Cumulative base tax (`base_tax`)

### Filing Statuses

- Single (S)
- Married Filing Jointly (MFJ)
- Married Filing Separately (MFS)
- Head of Household (HOH)
- Qualifying Surviving Spouse (QSS)

### Estimated Tax Calculation (`EstimatedTaxCalculation`)

Captures user inputs and calculated results for a tax estimate.

## User Input Fields

### SE Worksheet (Self-Employment Tax)

- Income subject to SE tax
- Expected Conservation Reserve Program (CRP) payments
- Expected wages (affects SS tax calculation)

### 1040-ES Worksheet (Estimated Tax)

- AGI (Adjusted Gross Income) estimate
- Deduction amount (Standard or Itemized)
- Qualified Business Income (QBI) deduction estimate
- Alternative Minimum Tax (AMT) estimate
- Credits estimate
- Other taxes estimate
- Prior year tax liability
- Income tax withheld estimate

## Current Limitations

The following tax scenarios are not currently supported:

- **Additional Medicare Tax**: The 0.9% surtax on wages/SE income exceeding $200k (Single/HOH) or $250k (MFJ) is not calculated
- **Net Investment Income Tax (NIIT)**: The 3.8% tax on investment income for high earners is not modeled
- **Safe Harbor 110% Rule**: For prior year AGI > $150k, safe harbor is 110% of prior year tax (not 100%); this distinction is not enforced
- **Farm vs. Non-Farm SE Income**: The SE Worksheet lines 1a/1b distinction is combined into a single field
- **Quarterly Payment Tracking**: Due dates and individual quarterly payments are not tracked

See [EVALUATION.md](EVALUATION.md) for detailed analysis and recommendations.

## Development

### Prerequisites

- Rust (edition 2024)
- SQLx CLI (for migrations): `cargo install sqlx-cli`

### Building

```powershell
cargo build
```

### Running Tests

```powershell
cargo test
```

Tests use an in-memory SQLite database with automatic migration.

### Database Migrations

The SQLite database schema and seed data are managed via SQLx migrations in `tax-db-sqlite/migrations/`.

## License

MIT
