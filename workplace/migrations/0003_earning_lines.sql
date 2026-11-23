-- Daily earnings are now the business an employee conducted, split across
-- admin-managed earning types. One row per type per employee-day.
--
-- Legacy day-rate/hourly entries are preserved: their base+overtime amount is
-- folded into `entries.base_minor` (exposed as `business_minor`) and recorded as
-- a single "Recorded day pay" line so old history stays reportable.

CREATE TABLE earning_types (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    active     INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE UNIQUE INDEX ux_earning_types_name_live
    ON earning_types (name COLLATE NOCASE)
    WHERE deleted_at IS NULL;

CREATE TABLE entry_lines (
    id           TEXT PRIMARY KEY,
    entry_id     TEXT NOT NULL REFERENCES entries (id) ON DELETE CASCADE,
    type_id      TEXT NOT NULL,
    type_name    TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    amount_minor INTEGER NOT NULL CHECK (amount_minor >= 0),
    position     INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL,
    UNIQUE (entry_id, type_id)
);

CREATE INDEX idx_entry_lines_entry ON entry_lines (entry_id);

INSERT INTO earning_types (id, name, active, sort_order, created_at, updated_at, deleted_at)
VALUES
    ('00000000-0000-4000-8000-000000000001', 'Recorded day pay', 0, 999, 0, 0, NULL),
    ('00000000-0000-4000-8000-000000000101', 'Sales', 1, 10, 0, 0, NULL),
    ('00000000-0000-4000-8000-000000000102', 'Service', 1, 20, 0, 0, NULL),
    ('00000000-0000-4000-8000-000000000103', 'Tips', 1, 30, 0, 0, NULL);

-- Fold legacy overtime into the business amount so gross/net stay unchanged.
UPDATE entries SET
    base_minor = base_minor + ot_minor,
    ot_minor = 0,
    day_fraction_bp = 10000,
    regular_minutes = 0,
    overtime_minutes = 0,
    ot_multiplier_bp = NULL,
    override_amount_minor = NULL
WHERE ot_minor <> 0
   OR overtime_minutes <> 0
   OR regular_minutes <> 0
   OR day_fraction_bp <> 10000
   OR ot_multiplier_bp IS NOT NULL
   OR override_amount_minor IS NOT NULL;

INSERT INTO entry_lines
    (id, entry_id, type_id, type_name, description, amount_minor, position, created_at)
SELECT
    lower(hex(randomblob(16))),
    e.id,
    '00000000-0000-4000-8000-000000000001',
    'Recorded day pay',
    '',
    e.base_minor,
    0,
    e.created_at
FROM entries e
WHERE e.base_minor > 0;
