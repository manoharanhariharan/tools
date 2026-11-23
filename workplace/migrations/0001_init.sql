-- Daybook initial schema.
--
-- Conventions
--  * Money is stored as INTEGER minor units (paise for INR). Never floats.
--  * Timestamps are INTEGER Unix milliseconds (UTC).
--  * work_date is TEXT 'YYYY-MM-DD' (civil date in the shop timezone).
--  * seq is a global, server-assigned, strictly increasing change counter used
--    by the sync pull/push protocol. Clients store it as base_seq.
--  * Rows are soft-deleted with deleted_at; they are never physically removed
--    so that offline devices can learn about tombstones.

CREATE TABLE meta (
    key       TEXT PRIMARY KEY,
    int_value INTEGER NOT NULL DEFAULT 0
);

INSERT INTO meta (key, int_value) VALUES ('seq', 0);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE INDEX idx_sessions_expires_at ON sessions (expires_at);

CREATE TABLE employees (
    id                       TEXT PRIMARY KEY,
    name                     TEXT NOT NULL,
    code                     TEXT,
    pay_basis                TEXT NOT NULL CHECK (pay_basis IN ('day_rate', 'hourly', 'fixed')),
    rate_minor               INTEGER NOT NULL CHECK (rate_minor >= 0),
    ot_multiplier_bp         INTEGER NOT NULL DEFAULT 15000
                             CHECK (ot_multiplier_bp BETWEEN 10000 AND 30000),
    standard_minutes_per_day INTEGER NOT NULL DEFAULT 480
                             CHECK (standard_minutes_per_day BETWEEN 60 AND 1440),
    note                     TEXT NOT NULL DEFAULT '',
    active                   INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at               INTEGER NOT NULL,
    updated_at               INTEGER NOT NULL,
    deleted_at               INTEGER,
    seq                      INTEGER NOT NULL
);

CREATE UNIQUE INDEX ux_employees_code_live
    ON employees (code) WHERE code IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX idx_employees_seq ON employees (seq);

CREATE TABLE entries (
    id                    TEXT PRIMARY KEY,
    employee_id           TEXT NOT NULL REFERENCES employees (id),
    work_date             TEXT NOT NULL CHECK (length(work_date) = 10),
    day_fraction_bp       INTEGER NOT NULL DEFAULT 10000
                          CHECK (day_fraction_bp BETWEEN 0 AND 20000),
    regular_minutes       INTEGER NOT NULL DEFAULT 0
                          CHECK (regular_minutes BETWEEN 0 AND 1440),
    overtime_minutes      INTEGER NOT NULL DEFAULT 0
                          CHECK (overtime_minutes BETWEEN 0 AND 1440),
    ot_multiplier_bp      INTEGER
                          CHECK (ot_multiplier_bp IS NULL
                                 OR ot_multiplier_bp BETWEEN 10000 AND 30000),
    override_amount_minor INTEGER
                          CHECK (override_amount_minor IS NULL
                                 OR override_amount_minor >= 0),
    bonus_minor           INTEGER NOT NULL DEFAULT 0 CHECK (bonus_minor >= 0),
    advance_minor         INTEGER NOT NULL DEFAULT 0 CHECK (advance_minor >= 0),
    deduction_minor       INTEGER NOT NULL DEFAULT 0 CHECK (deduction_minor >= 0),
    base_minor            INTEGER NOT NULL,
    ot_minor              INTEGER NOT NULL,
    gross_minor           INTEGER NOT NULL,
    net_minor             INTEGER NOT NULL,
    note                  TEXT NOT NULL DEFAULT '',
    created_at            INTEGER NOT NULL,
    updated_at            INTEGER NOT NULL,
    deleted_at            INTEGER,
    seq                   INTEGER NOT NULL
);

CREATE UNIQUE INDEX ux_entries_employee_date_live
    ON entries (employee_id, work_date) WHERE deleted_at IS NULL;
CREATE INDEX idx_entries_work_date ON entries (work_date);
CREATE INDEX idx_entries_employee ON entries (employee_id);
CREATE INDEX idx_entries_seq ON entries (seq);
