-- Audit trail: an append-only log of every data change, viewable by admins.
--
-- Every create/update/delete on employees, entries and earning types, plus
-- admin password changes and employee PIN set/clear, writes one row here inside
-- the same transaction as the change, so a change is never recorded without its
-- audit entry (and vice versa).

CREATE TABLE audit_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at INTEGER NOT NULL,
    actor_role TEXT NOT NULL,               -- 'admin' | 'employee'
    actor_id   TEXT,                        -- employee id for employee actors
    actor_name TEXT NOT NULL DEFAULT '',
    entity     TEXT NOT NULL,               -- employee | entry | earning_type | password | employee_pin
    entity_id  TEXT NOT NULL DEFAULT '',    -- the affected row id ('' for credential events)
    action     TEXT NOT NULL,               -- create | update | delete | set | clear | change
    summary    TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_audit_log_created_at ON audit_log (created_at);
CREATE INDEX idx_audit_log_entity ON audit_log (entity, entity_id);
