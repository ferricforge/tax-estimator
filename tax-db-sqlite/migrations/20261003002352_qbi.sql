-- Form 8995 (Qualified Business Income Deduction, Simplified Computation).
-- Belongs to one tax estimate; at most one row per estimate.
CREATE TABLE qbi (
    tax_estimate_id INTEGER PRIMARY KEY,
    -- User-provided values (Form 8995 inputs)
    qbi_loss_carryforward DECIMAL(12,2) NOT NULL DEFAULT 0,      -- Line 3
    reit_ptp_income DECIMAL(12,2) NOT NULL DEFAULT 0,            -- Line 6
    reit_ptp_loss_carryforward DECIMAL(12,2) NOT NULL DEFAULT 0, -- Line 7
    taxable_income_before_qbi DECIMAL(12,2),                     -- Line 11
    net_capital_gain DECIMAL(12,2) NOT NULL DEFAULT 0,           -- Line 12
    -- Calculated values
    calculated_qbi_deduction DECIMAL(12,2),                      -- Line 15
    calculated_qbi_loss_carryforward DECIMAL(12,2),              -- Line 16
    calculated_reit_ptp_loss_carryforward DECIMAL(12,2),         -- Line 17
    FOREIGN KEY (tax_estimate_id) REFERENCES tax_estimate(id) ON DELETE CASCADE
);

-- Form 8995, Line 1: one row per trade, business, or aggregation.
-- Belongs to one qbi row.
CREATE TABLE qbi_business (
    tax_estimate_id INTEGER NOT NULL,
    line_number INTEGER NOT NULL,                                -- 1 is Line 1i
    business_name VARCHAR(100) NOT NULL,                         -- column (a)
    taxpayer_id VARCHAR(11) NOT NULL,                            -- column (b)
    qualified_business_income DECIMAL(12,2) NOT NULL DEFAULT 0,  -- column (c)
    PRIMARY KEY (tax_estimate_id, line_number),
    FOREIGN KEY (tax_estimate_id) REFERENCES qbi(tax_estimate_id) ON DELETE CASCADE
);
