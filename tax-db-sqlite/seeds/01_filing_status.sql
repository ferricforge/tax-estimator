-- Seed filing statuses
INSERT OR IGNORE INTO filing_status (id, status_code, status_name) VALUES
(1, 'S', 'Single'),
(2, 'MFJ', 'Married Filing Jointly'),
(3, 'MFS', 'Married Filing Separately'),
(4, 'HOH', 'Head of Household'),
(5, 'QSS', 'Qualifying Surviving Spouse');
