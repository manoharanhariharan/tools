# Sync protocol

Daybook is offline-first. Phones write locally first and reconcile with one
server; the protocol is deliberately small and explicit about conflicts.

## Records and invariants

Every synced record (`employee`, `entry`) has:

| Field | Meaning |
| --- | --- |
| `id` | UUID. Browsers generate v4; the server generates v7 for its own writes. Clients may choose ids for offline creates. |
| `created_at` | Unix ms, UTC. Preserved across updates. |
| `updated_at` | Unix ms, UTC, server-assigned on every applied change. |
| `deleted_at` | `null`, or the tombstone timestamp. Rows are never physically deleted. |
| `seq` | Server-assigned global sequence. Unique, strictly increasing. |

A `entry` is one employee-day sheet. Its earning rows are embedded in the
payload as `lines` (`type_id`, `type_name`, `description`, `amount_minor`); they
do not have their own sequence because the whole day sheet is the sync unit.
`type_name` is a snapshot taken by the server when the sheet is saved.

`seq` is a **single counter across both tables** (stored in `meta.seq`). Every
applied change takes the next value. This makes one cursor sufficient and keeps
pulls ordered and gapless.

Soft deletes exist so a device that was offline for a month still learns that a
row disappeared. Entry queries and reports exclude tombstones; historical
earnings for deleted employees remain reportable.

## Earning type catalogue (not synced)

Earning types are admin configuration, not day data. They are managed online:

- `GET /api/earning-types`
- `POST /api/earning-types` — `{ "name": "Delivery", "active": true }`
- `PATCH /api/earning-types/{id}`
- `DELETE /api/earning-types/{id}` (soft delete)

Phones cache the list in IndexedDB and refresh it after every successful sync.
The catalogue therefore works offline for entering day sheets, but changing it
needs a connection. Historical rows keep the type name snapshot, so renames and
deletes never rewrite past money.

## Authentication and request requirements

All endpoints require the admin session cookie
(`daybook_session`, set by `POST /api/auth/login`) and every mutating request
must include:

```
X-Daybook-Client: daybook-web/1
```

Missing header → `403`. Missing/expired cookie → `401`. There is no API token
scheme; this is a single-admin LAN application.

## Discover the cursor

`GET /api/sync/state`

```json
{ "cursor": 42, "server_time": 1763856000000 }
```

A device that has never synced starts from `since=0`.

## Pull

`GET /api/sync/pull?since=<seq>&limit=<1..500>` (default limit 200)

Reads happen in one SQLite read transaction, so the cursor and the rows always
come from the same snapshot.

```json
{
  "cursor": 57,
  "latest": 57,
  "has_more": false,
  "employees": [ { "id": "...", "seq": 51, "...": "..." } ],
  "entries": [
    {
      "id": "...",
      "seq": 57,
      "employee_id": "...",
      "work_date": "2026-11-23",
      "business_minor": 80000,
      "bonus_minor": 10000,
      "gross_minor": 90000,
      "advance_minor": 5000,
      "deduction_minor": 0,
      "net_minor": 85000,
      "lines": [
        { "type_id": "…sales", "type_name": "Sales", "description": "counter", "amount_minor": 50000 },
        { "type_id": "…service", "type_name": "Service", "description": "", "amount_minor": 30000 }
      ],
      "...": "..."
    }
  ]
}
```

- Rows are ordered by `seq` ascending across both entities.
- `cursor` is the last sequence *included*; pass it as the next `since`.
- `has_more` means more rows exist after `cursor`; keep paging.
- `latest` is the server's current sequence. If `latest < since`, the server was
  restored from an older backup: reset to `since=0` and rebuild non-pending
  local rows.
- Soft-deleted rows are included with `deleted_at` set.
- A pull response never mixes rows newer than its `cursor`, so a client can
  persist the cursor after applying the page and resume safely.

Pagination stops after 60 pages (12,000 changes) per client run; the UI shows a
sync error rather than spinning forever if something is wrong.

## Push

`POST /api/sync/push`

```json
{
  "device_id": "web-…",
  "changes": [
    {
      "entity": "employee",
      "id": "6f1c…",
      "base_seq": 0,
      "deleted": false,
      "force": false,
      "data": { "name": "Asha", "code": "EMP-1", "note": "", "active": true }
    },
    {
      "entity": "entry",
      "id": "0b7e…",
      "base_seq": 0,
      "deleted": false,
      "force": false,
      "data": {
        "employee_id": "6f1c…",
        "work_date": "2026-11-23",
        "lines": [
          { "type_id": "…sales", "description": "counter", "amount_minor": 50000 },
          { "type_id": "…service", "description": "repair", "amount_minor": 30000 }
        ],
        "bonus_minor": 10000,
        "advance_minor": 5000,
        "deduction_minor": 0,
        "note": ""
      }
    }
  ]
}
```

- Maximum 200 changes per request.
- `base_seq` is the server sequence the device last saw: `0` for a create.
- `force: true` overrides a conflict (per change).
- `deleted: true` is a tombstone request; `data` may be omitted.
- **Employees are applied before entries**, regardless of array order, so an
  offline bundle can contain a new employee and their first day sheet.
- The server ignores client totals and resolves `type_name` from the catalogue.
  It recomputes `business = Σ lines`, `gross = business + bonus`,
  `net = gross − advance − deduction`.

Response:

```json
{
  "cursor": 58,
  "results": [
    {
      "entity": "entry",
      "id": "0b7e…",
      "status": "applied",
      "server": { "id": "0b7e…", "seq": 58, "business_minor": 80000, "net_minor": 85000, "...": "..." }
    },
    {
      "entity": "entry",
      "id": "9a12…",
      "status": "conflict",
      "conflict": "stale",
      "server": { "id": "9a12…", "seq": 50, "...": "..." }
    },
    {
      "entity": "employee",
      "id": "77aa…",
      "status": "invalid",
      "error": {
        "code": "validation_failed",
        "message": "some fields are invalid",
        "fields": [{ "field": "type_id", "message": "unknown earning type" }]
      }
    }
  ]
}
```

Per-change statuses:

| Status | Meaning | Client action |
| --- | --- | --- |
| `applied` | Written; `server` contains the authoritative row. | Store server row, remove from outbox. |
| `noop` | Already equal / already deleted / retry of an applied change. | Store server row (if present), remove from outbox. |
| `conflict` | Needs a user decision; `conflict` names the reason and `server` is the current row. | Move to the conflict queue; do not retry blindly. |
| `invalid` | Failed validation; `error.fields` pinpoints the problem. | Keep for editing or discard. |

Invalid or conflicting items **do not roll back** other items in the batch; the
whole batch still commits atomically at the database level, and sequence numbers
are consumed only by applied changes.

### Decision table

`existing` is the server row with the same id (including tombstones).

| Situation | Result |
| --- | --- |
| No existing row, `base_seq = 0` | Apply (create) |
| No existing row, `base_seq > 0`, not forced | `conflict: missing_on_server` |
| No existing row, delete | `noop` |
| Existing row, `seq == base_seq` | Apply |
| Existing row, stale, identical content | `noop` (safe retry) |
| Existing row, stale, different content, forced | Apply |
| Existing row, stale, different content | `conflict: stale` (or `already_exists` when `base_seq = 0`) |
| Existing deleted row, delete | `noop` |
| Existing deleted row, recreate, not forced | `conflict: stale` |

Additional constraints produce actionable conflicts:

- `duplicate_code` — another live employee already uses that code.
- `duplicate_entry_date` — another live entry already occupies that
  employee+date. `force` cannot bypass this; edit the date or use the existing
  row.
- `employee_not_found` / `employee_unavailable` — an entry references a missing
  or deleted employee.

Force on a deleted row resurrects it and preserves the original `created_at`.

## Server reset / restored backup

Clients store the last applied `cursor`. If the server returns
`latest < since`, the client:

1. deletes all local employees/entries that are **not** in the outbox,
2. sets its cursor to 0,
3. pulls from scratch.

Pending local edits are kept and pushed afterwards. This means restoring an old
backup cannot leave ghost rows on phones.

## Soft delete semantics

| Action | Server | Other devices |
| --- | --- | --- |
| Delete entry | sets `deleted_at`, bumps `seq` | row disappears from daily screens; reports exclude it |
| Delete employee | sets `deleted_at`, bumps `seq` | disappears from employee pickers; existing entries and reports remain |
| Create entry for deleted employee | rejected (`invalid`) | — |
| Delete an unsynced local create | client removes it and its outbox item; no server tombstone | — |

## Scope

Only `employees` and `entries` are synced. Settings, the admin password hash,
sessions and the report cache stay server-side. Report totals are always
computed from server data on demand.

## Worked example: offline create

1. Phone is offline. User adds employee `E` and entry `X` for today.
   - IndexedDB gets both rows (marked pending) and two outbox changes with
     `base_seq: 0`, in one transaction each.
2. Phone reconnects. `POST /api/sync/push` with both changes.
3. Server applies `E` then `X`, recomputes `X`'s amounts, assigns `seq` 12 and 13.
4. Response: both `applied` with full server rows.
5. Phone replaces its optimistic rows with the server rows and deletes the
   outbox items.
6. `GET /api/sync/pull?since=<previous cursor>` returns the same rows plus
   anything other devices changed; the UI updates.

If step 2's response is lost, the retry returns `noop` for both; no duplicate
rows and no sequence burn.
