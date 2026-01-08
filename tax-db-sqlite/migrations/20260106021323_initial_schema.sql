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

CREATE TABLE tax_estimate (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tax_year INTEGER NOT NULL,

    -- User-provided values (1040-ES Worksheet inputs)
    filing_status_id INTEGER NOT NULL,
    expected_agi DECIMAL(12,2) NOT NULL DEFAULT 0,
    expected_deduction DECIMAL(12,2) NOT NULL DEFAULT 0,
    expected_qbi_deduction DECIMAL(12,2),
    expected_amt DECIMAL(12,2),
    expected_credits DECIMAL(12,2),
    expected_other_taxes DECIMAL(12,2),
    expected_withholding DECIMAL(12,2),
    prior_year_tax DECIMAL(12,2),

    -- User-provided values (SE Worksheet inputs)
    se_income DECIMAL(12,2),
    expected_crp_payments DECIMAL(12,2),
    expected_wages DECIMAL(12,2),

    -- Calculated values
    calculated_se_tax DECIMAL(12,2),
    calculated_total_tax DECIMAL(12,2),
    calculated_required_payment DECIMAL(12,2),

    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tax_year) REFERENCES tax_year_config(tax_year),
    FOREIGN KEY (filing_status_id) REFERENCES filing_status(id)
);
