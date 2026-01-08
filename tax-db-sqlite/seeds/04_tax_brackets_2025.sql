-- Seed 2025 tax brackets (Schedule X - Single)
INSERT OR IGNORE INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 1, 0, 11925, 0.10, 0),
(2025, 1, 11925, 48475, 0.12, 1192.50),
(2025, 1, 48475, 103350, 0.22, 5578.50),
(2025, 1, 103350, 197300, 0.24, 17651),
(2025, 1, 197300, 250525, 0.32, 40199),
(2025, 1, 250525, 626350, 0.35, 57231),
(2025, 1, 626350, NULL, 0.37, 188769.75);

-- Seed 2025 tax brackets (Schedule Y-1 - MFJ)
INSERT OR IGNORE INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 2, 0, 23850, 0.10, 0),
(2025, 2, 23850, 96950, 0.12, 2385),
(2025, 2, 96950, 206700, 0.22, 11157),
(2025, 2, 206700, 394600, 0.24, 35302),
(2025, 2, 394600, 501050, 0.32, 80398),
(2025, 2, 501050, 751600, 0.35, 114462),
(2025, 2, 751600, NULL, 0.37, 202154.50);

-- Seed 2025 tax brackets (Schedule Y-2 - MFS)
INSERT OR IGNORE INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 3, 0, 11925, 0.10, 0),
(2025, 3, 11925, 48475, 0.12, 1192.50),
(2025, 3, 48475, 103350, 0.22, 5578.50),
(2025, 3, 103350, 197300, 0.24, 17651),
(2025, 3, 197300, 250525, 0.32, 40199),
(2025, 3, 250525, 375800, 0.35, 57231),
(2025, 3, 375800, NULL, 0.37, 101077.25);

-- Seed 2025 tax brackets (Schedule Z - HOH)
INSERT OR IGNORE INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 4, 0, 17000, 0.10, 0),
(2025, 4, 17000, 64850, 0.12, 1700),
(2025, 4, 64850, 103350, 0.22, 7442),
(2025, 4, 103350, 197300, 0.24, 15912),
(2025, 4, 197300, 250500, 0.32, 38460),
(2025, 4, 250500, 626350, 0.35, 55484),
(2025, 4, 626350, NULL, 0.37, 187032);

-- Seed 2025 tax brackets (QSS - same as MFJ)
INSERT OR IGNORE INTO tax_brackets (tax_year, filing_status_id, min_income, max_income, tax_rate, base_tax) VALUES
(2025, 5, 0, 23850, 0.10, 0),
(2025, 5, 23850, 96950, 0.12, 2385),
(2025, 5, 96950, 206700, 0.22, 11157),
(2025, 5, 206700, 394600, 0.24, 35302),
(2025, 5, 394600, 501050, 0.32, 80398),
(2025, 5, 501050, 751600, 0.35, 114462),
(2025, 5, 751600, NULL, 0.37, 202154.50);
