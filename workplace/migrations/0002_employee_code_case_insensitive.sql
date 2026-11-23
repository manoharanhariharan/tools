-- Employee codes are compared case-insensitively by the UI; enforce the same
-- rule in the database and in lookups so "EMP-1" and "emp-1" cannot coexist.

DROP INDEX IF EXISTS ux_employees_code_live;

CREATE UNIQUE INDEX ux_employees_code_live
    ON employees (code COLLATE NOCASE)
    WHERE code IS NOT NULL AND deleted_at IS NULL;
