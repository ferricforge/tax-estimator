CREATE TABLE tax_year_config (
    tax_year INTEGER PRIMARY KEY,
    ss_wage_max DECIMAL(12,2) NOT NULL,
    ss_tax_rate DECIMAL(5,4) NOT NULL,
    medicare_tax_rate DECIMAL(5,4) NOT NULL,
    se_tax_deductible_percentage DECIMAL(5,4) NOT NULL,
    se_deduction_factor DECIMAL(5,4) NOT NULL,
    required_payment_threshold DECIMAL(12,2) NOT NULL
);

CREATE TABLE filing_status (
    id INTEGER PRIMARY KEY,
    status_code VARCHAR(3) NOT NULL UNIQUE,
    status_name VARCHAR(50) NOT NULL
);

CREATE TABLE standard_deductions (
    tax_year INTEGER NOT NULL,
    filing_status_id INTEGER NOT NULL,
    amount DECIMAL(12,2) NOT NULL,
    PRIMARY KEY (tax_year, filing_status_id),
    FOREIGN KEY (tax_year) REFERENCES tax_year_config(tax_year),
    FOREIGN KEY (filing_status_id) REFERENCES filing_status(id)
);

CREATE TABLE tax_brackets (
    tax_year INTEGER NOT NULL,
    filing_status_id INTEGER NOT NULL,
    min_income DECIMAL(12,2) NOT NULL,
    max_income DECIMAL(12,2),
    tax_rate DECIMAL(5,4) NOT NULL,
    base_tax DECIMAL(12,2) NOT NULL,
    PRIMARY KEY (tax_year, filing_status_id, min_income),
    FOREIGN KEY (tax_year) REFERENCES tax_year_config(tax_year),
    FOREIGN KEY (filing_status_id) REFERENCES filing_status(id)
);

CREATE TABLE estimated_tax_calculation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tax_year INTEGER NOT NULL,
    filing_status_id INTEGER NOT NULL,
    expected_agi DECIMAL(12,2) NOT NULL DEFAULT 0,
    expected_deduction DECIMAL(12,2) NOT NULL DEFAULT 0,
    expected_qbi_deduction DECIMAL(12,2),
    expected_amt DECIMAL(12,2),
    expected_credits DECIMAL(12,2),
    expected_other_taxes DECIMAL(12,2),
    prior_year_tax DECIMAL(12,2),
    expected_withholding DECIMAL(12,2),
    se_income DECIMAL(12,2),
    expected_crp_payments DECIMAL(12,2),
    expected_wages DECIMAL(12,2),
    calculated_se_tax DECIMAL(12,2),
    calculated_total_tax DECIMAL(12,2),
    calculated_required_payment DECIMAL(12,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tax_year) REFERENCES tax_year_config(tax_year),
    FOREIGN KEY (filing_status_id) REFERENCES filing_status(id)
);

-- Seed filing statuses
INSERT INTO filing_status (id, status_code, status_name) VALUES
(1, 'S', 'Single'),
(2, 'MFJ', 'Married Filing Jointly'),
(3, 'MFS', 'Married Filing Separately'),
(4, 'HOH', 'Head of Household'),
(5, 'QSS', 'Qualifying Surviving Spouse');

-- Seed 2025 tax year config
INSERT INTO tax_year_config (
    tax_year, ss_wage_max, ss_tax_rate, medicare_tax_rate,
    se_tax_deductible_percentage, se_deduction_factor,
    required_payment_threshold
) VALUES (
    2025, 176100.00, 0.124, 0.029,
    0.9235, 0.50,
    1000.00
);

-- Seed 2025 standard deductions
INSERT INTO standard_deductions (tax_year, filing_status_id, amount) VALUES
(2025, 1, 15000.00),
(2025, 2, 30000.00),
(2025, 3, 15000.00),
(2025, 4, 22500.00),
(2025, 5, 30000.00);

-- Seed 2025 tax brackets (Schedule X - Single)
INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 1, 0, 11925, 0.10, 0),
(2025, 1, 11925, 48475, 0.12, 1192.50),
(2025, 1, 48475, 103350, 0.22, 5578.50),
(2025, 1, 103350, 197300, 0.24, 17651),
(2025, 1, 197300, 250525, 0.32, 40199),
(2025, 1, 250525, 626350, 0.35, 57231),
(2025, 1, 626350, NULL, 0.37, 188769.75);

-- Seed 2025 tax brackets (Schedule Y-1 - MFJ)
INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 2, 0, 23850, 0.10, 0),
(2025, 2, 23850, 96950, 0.12, 2385),
(2025, 2, 96950, 206700, 0.22, 11157),
(2025, 2, 206700, 394600, 0.24, 35302),
(2025, 2, 394600, 501050, 0.32, 80398),
(2025, 2, 501050, 751600, 0.35, 114462),
(2025, 2, 751600, NULL, 0.37, 202154.50);

-- Seed 2025 tax brackets (Schedule Y-2 - MFS)
INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 3, 0, 11925, 0.10, 0),
(2025, 3, 11925, 48475, 0.12, 1192.50),
(2025, 3, 48475, 103350, 0.22, 5578.50),
(2025, 3, 103350, 197300, 0.24, 17651),
(2025, 3, 197300, 250525, 0.32, 40199),
(2025, 3, 250525, 375800, 0.35, 57231),
(2025, 3, 375800, NULL, 0.37, 101077.25);

-- Seed 2025 tax brackets (Schedule Z - HOH)
INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 4, 0, 17000, 0.10, 0),
(2025, 4, 17000, 64850, 0.12, 1700),
(2025, 4, 64850, 103350, 0.22, 7442),
(2025, 4, 103350, 197300, 0.24, 15912),
(2025, 4, 197300, 250500, 0.32, 38460),
(2025, 4, 250500, 626350, 0.35, 55484),
(2025, 4, 626350, NULL, 0.37, 187032);

-- Seed 2025 tax brackets (QSS - same as MFJ)
INSERT INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 5, 0, 23850, 0.10, 0),
(2025, 5, 23850, 96950, 0.12, 2385),
(2025, 5, 96950, 206700, 0.22, 11157),
(2025, 5, 206700, 394600, 0.24, 35302),
(2025, 5, 394600, 501050, 0.32, 80398),
(2025, 5, 501050, 751600, 0.35, 114462),
(2025, 5, 751600, NULL, 0.37, 202154.50);
