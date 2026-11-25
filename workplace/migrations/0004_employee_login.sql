-- Employee login: per-employee PIN credentials and role-scoped sessions.
--
-- * employees.pin_hash stores an Argon2 hash of the employee's 4-digit PIN.
--   It is never serialised to clients or synced.
-- * sessions gains role/employee_id/name so the auth middleware can rebuild an
--   identity (admin vs employee) without joining employees on every request.

ALTER TABLE employees ADD COLUMN pin_hash TEXT;

ALTER TABLE sessions ADD COLUMN role TEXT NOT NULL DEFAULT 'admin';
ALTER TABLE sessions ADD COLUMN employee_id TEXT;
ALTER TABLE sessions ADD COLUMN name TEXT NOT NULL DEFAULT '';

CREATE INDEX idx_sessions_employee_id ON sessions (employee_id);
