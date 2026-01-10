-- Add minimum self-employment threshold column
ALTER TABLE tax_year_config ADD COLUMN min_se_threshold DECIMAL(12,2) NOT NULL DEFAULT 400.00;
