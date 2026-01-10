-- Seed 2025 tax year config
INSERT OR IGNORE INTO tax_year_config (
    tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
    se_tax_deductible_percentage, se_deduction_factor,
    required_payment_threshold, min_se_threshold
) VALUES (
    2025, 176100.00, 0.124, 0.029,
    0.9235, 0.50,
    1000.00, 400.00
);
