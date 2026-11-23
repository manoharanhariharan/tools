# Daybook

In-house daily employee earnings — a single self-hosted Rust service with a
mobile-first offline PWA and authoritative reports. No Google Play, no App Store,
no cloud dependency.

- **Employees & earning types** — an admin-managed catalogue (Sales, Service,
  Tips, …). Each employee-day records one amount per type, plus a free-text
  description and bonus/advance/deduction adjustments.
- **Daily earnings** — the business an employee generated is the sum of the day's
  type rows; final money is business + bonus − advance − deduction. All amounts
  are recomputed and snapshotted on the server.
- **Offline-first mobile access** — one responsive web app installed to the home
  screen on Android and iPhone. Day sheets are saved in IndexedDB and pushed when
  back on the network.
- **Sync with explicit conflicts** — a global change cursor, idempotent retries,
  tombstones for deletes, and a review screen when two devices edit the same row.
- **Reports** — on-screen summaries, printable HTML, server-side PDF, and CSV,
  for any range up to 366 days.
- **In-house by design** — one admin password, SQLite on your machine, LAN or
  VPN access only, ₹ / INR and IST dates.

---

## Requirements

| Component | Requirement |
| --- | --- |
| Build | Rust 1.88+ (`cargo`), tested on Rust 1.99 |
| Runtime | Windows 10/11, Linux, or macOS |
| Database | SQLite (bundled; nothing to install) |
| Phones | Any modern Android/iPhone browser (Chrome/Safari); no store account |
| Network | Phone and server on the same Wi-Fi/LAN, or both on a VPN such as Tailscale |

Node.js is only needed for the JavaScript unit tests; the PWA has **no build
step** and no npm dependencies.

---

## Quick start

```powershell
cargo build --release
.\target\release\daybook.exe cert init --host 192.168.1.20,localhost --out certs
$env:DAYBOOK_TLS_CERT = "certs\daybook-cert.pem"
$env:DAYBOOK_TLS_KEY  = "certs\daybook-key.pem"
.\target\release\daybook.exe serve
```

On the first run the server prints a generated admin password **once**; write it
down. If you set `DAYBOOK_ADMIN_PASSWORD` before the very first start, that value
is used instead and nothing is printed. Then install the generated CA certificate
on each phone (see [docs/mobile-install.md](docs/mobile-install.md)) and open
`https://192.168.1.20:8787`.

On Windows you can start it by double-clicking **`start-daybook.bat`**. It uses
the release binary when present, uses `certs\` automatically when those files
exist, and keeps the console open after shutdown. Set `DAYBOOK_NO_TLS=1` before
running it to force plain HTTP.

Plain HTTP (no certificate) is fine for local development:

```powershell
$env:DAYBOOK_BIND = "127.0.0.1:8787"   # optional; default is 0.0.0.0:8787
.\target\debug\daybook.exe serve
# open http://localhost:8787
```

Default data directory: `%LOCALAPPDATA%\Daybook` on Windows,
`$XDG_DATA_HOME/daybook` (or `~/.local/share/daybook`) elsewhere. Override with
`DAYBOOK_DATA_DIR` or `--data-dir`.

Reset the admin password (also revokes every session):

```powershell
.\target\release\daybook.exe admin set-password
```

---

## Daily earnings model

Employees are people, not rates. The shop defines **earning types** once in the
Employees tab (seeded with Sales, Service and Tips; rename, deactivate, add or
delete as needed). Types are server configuration, cached on each phone so the
day form works offline.

For each employee and day:

- The form shows **one row per active earning type**: an optional free-text
  description and an amount in ₹.
- The same type appears only once per day; the amount is that type's total.
- **Business** is the sum of all earning-type amounts.
- **Bonus** is added; **advance** and **deduction** are subtracted:
  `gross = business + bonus`, `final money = gross − advance − deduction`.
  Advances may exceed gross, producing a negative final amount for that day.

Rules that matter:

- **The server is authoritative.** Clients may send totals, but every save (REST
  or sync) recomputes business, gross and final money from the rows.
- **Type names are snapshotted.** Renaming, deactivating or deleting a type never
  rewrites historical day sheets; old rows keep the name used at the time.
- **Money is integer paise.** No floating point touches payroll.
- **Dates are civil IST dates** (`YYYY-MM-DD`); “today” is derived from
  Asia/Kolkata, not the device timezone.
- **Employee codes are unique case-insensitively** (`EMP-1` = `emp-1`).
- **No hours, hourly rates, day rates or overtime.** The app records business
  generated, not time worked.
- Existing data from the earlier rate/hour model is preserved: each old day is
  folded into a single inactive “Recorded day pay” type row so history remains
  reportable.

---

## Mobile access (no Play Store / App Store)

The recommended and only client is the **PWA served by the Rust binary**:

- one codebase, one deployment;
- installs to the home screen on Android/iPhone;
- no APK signing, no developer accounts, no store review, no updates to push;
- offline-capable via a service worker + IndexedDB.

**HTTPS is mandatory on a LAN address.** Browsers only allow service workers,
installation and offline caching on secure origins. `http://192.168.x.x` is not
secure, so the app will run online but without offline caching or install
promotion.

Two supported ways to get HTTPS:

1. **Local CA (default, no internet):** `daybook cert init` creates a local CA and
   a 397-day server certificate. Install `daybook-ca.crt` once on each phone.
2. **Tailscale HTTPS:** put the server and phones on a tailnet and use Tailscale
   Serve/certificates for a publicly trusted certificate (no CA install). This is
   also the simplest way to allow off-site access.

Step-by-step instructions, including Android/iOS trust settings and firewall
notes: [docs/mobile-install.md](docs/mobile-install.md).

A sideloaded Android APK or Rust FFI core was deliberately **not** built: it adds
signing/build maintenance without improving the in-house workflow. The PWA is
installable, works offline, and updates with the server.

---

## Sync

Records carry:

- a UUID (`v7` for server-created rows, `v4` from browsers),
- `created_at` / `updated_at` (Unix ms, UTC),
- `deleted_at` for soft deletes,
- a server-assigned global `seq` used as the sync cursor.

Clients keep an **outbox**: a local write is applied optimistically to IndexedDB
and queued in the same transaction. The sync engine pushes the outbox, then pulls
everything after its stored cursor.

- Retries are idempotent: an already-applied change comes back as `noop`.
- Pulls never overwrite rows that still have pending local changes.
- A stale edit becomes an explicit **conflict** with both versions; the user
  chooses “Keep my change” (force) or “Use server version”.
- Deletes are tombstones, so every device learns about them. Historical entries
  of deleted employees still appear in reports.
- The server reorders a batch so employees are applied before entries.
- Reports are server-generated, so the Reports tab needs a connection; daily
  entry and employee management work fully offline.

Full protocol, decision table, and examples:
[docs/sync-protocol.md](docs/sync-protocol.md).

---

## Reports

Every report is computed from live (non-deleted) day sheets in the requested
range. Each report shows **business generated**, bonus, advances, deductions,
**final money**, and a **business-by-earning-type** breakdown.

Earning-type configuration uses `GET/POST/PATCH/DELETE /api/earning-types` and
needs a connection; phones cache the list for offline day entry.

| Output | Endpoint | Notes |
| --- | --- | --- |
| JSON summary | `GET /api/reports/summary?from=&to=&employee_id=` | totals, per-employee rows, per-day rows, type breakdown |
| Printable HTML | `GET /api/reports/print.html?...&layout=` | self-contained, print CSS; open in a tab |
| PDF | `GET /api/reports/report.pdf?...&layout=` | A4, paginated, totals, ₹ via embedded font |
| CSV | `GET /api/reports/export.csv?...&layout=` | UTF-8 BOM, CRLF, plain decimals |

Layouts: `employee` (default), `daily`, `entries` (one row per employee-day with
its earning detail).

- The Reports tab offers **From/To date fields** for any range plus a month
  quick-pick; changing either field refreshes the on-screen summary.
- Maximum range: **366 days** per request.
- `from`/`to` default to the current IST month.
- PDF uses a system TrueType font with a ₹ glyph when available (Segoe UI on
  Windows, DejaVu/Liberation/Noto on Linux). If none is found it falls back to
  built-in Helvetica and renders `Rs`; set `DAYBOOK_PDF_FONT` to a `.ttf` to
  control this explicitly.
- Dollar-precision rules: components are rounded once, half-up, in paise.

---

## Configuration

All settings are environment variables (CLI flags override where noted).

| Variable | Default | Meaning |
| --- | --- | --- |
| `DAYBOOK_BIND` | `0.0.0.0:8787` | Listen address (also `serve --bind`) |
| `DAYBOOK_DATA_DIR` | platform data dir | SQLite database location (also `--data-dir`) |
| `DAYBOOK_TLS_CERT` | — | Server certificate PEM (leaf + CA chain) |
| `DAYBOOK_TLS_KEY` | — | Server private key PEM (must be set with the cert) |
| `DAYBOOK_SESSION_TTL_DAYS` | `30` | Admin session lifetime, 1–365 |
| `DAYBOOK_ADMIN_PASSWORD` | — | Only used on first run to seed the password |
| `DAYBOOK_PDF_FONT` | auto-detect | TrueType font used for PDFs |
| `DAYBOOK_WEB_DIR` | embedded | Development only: serve UI from disk |
| `RUST_LOG` | `daybook=info,tower_http=info` | Log filter |

CLI:

```
daybook serve [--bind ADDR]
daybook admin set-password
daybook cert init [--host HOST]... [--out DIR]
daybook doctor
```

`cert init` without `--host` auto-detects the primary LAN IPv4 plus localhost and
the machine name. Re-run it if the server IP changes (add `--force`); devices must
re-trust a new CA only if you generate a new one.

---

## Security model

- **Single admin account.** The password is hashed with Argon2id and stored
  server-side. Login is throttled per IP (5 failures / 5 minutes, bounded memory).
- **Sessions** are 256-bit opaque tokens; only their SHA-256 hashes are stored.
  Cookies are `HttpOnly`, `SameSite=Lax`, and `Secure` when TLS is enabled.
- **CSRF** is blocked by requiring an `X-Daybook-Client` header on every mutating
  API call (impossible for a cross-site form; cross-origin fetches require a
  preflight this server never approves) plus `SameSite` cookies.
- **Boundaries:** no third-party scripts, no telemetry, no external fonts/CDNs,
  strict CSP, `X-Frame-Options: DENY`, `nosniff`.
- **Transport:** deploy on a trusted LAN or VPN. Plain HTTP on a LAN is
  readable by anyone on that network; prefer the generated HTTPS setup.
- **Secrets at rest:** protect the data directory and `certs/daybook-key.pem`.
  The CA private key is never written to disk. The database is not encrypted at
  rest — use OS disk encryption if that matters to you.
- Reset the admin password if a phone is lost; all sessions are revoked.

This is a small in-house tool, not a certified payroll or accounting system.

---

## Backups and upgrades

See also [docs/operations.md](docs/operations.md) for Windows Task Scheduler,
systemd, firewall, monitoring and renewal notes.

**Backup (safe online):**

```bash
sqlite3 "$DATA_DIR/daybook.db" ".backup 'daybook-backup.db'"
```

If `sqlite3` is unavailable, stop the server and copy `daybook.db` together with
`daybook.db-wal` and `daybook.db-shm`. A lone `.db` file copied while WAL has
uncommitted pages is not a valid backup.

**Restore:** stop the server, replace the database (remove stale `-wal`/`-shm`),
start again. If a device's pull cursor is ahead of the restored server, the PWA
detects this and rebuilds its non-pending rows automatically.

**Upgrade:** replace the binary, bump `VERSION` in `web/sw.js`, restart. Schema
migrations run automatically at startup. The UI prompts installed clients to
reload when a new service worker is ready.

**Certificates:** `cert init` refuses to overwrite an existing set. When a new
certificate is required (IP change, expiry), run it with `--force` and re-install
the new CA on every phone.

---

## Development

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                      # unit + integration tests
node --test tests/js/*.test.mjs # PWA logic tests (Node 20+)
cargo run -- serve --bind 127.0.0.1:8787
```

For live UI edits without rebuilding:

```powershell
$env:DAYBOOK_WEB_DIR = "$PWD\web"
cargo run -- serve --bind 127.0.0.1:8787
```

Debug builds read UI assets from `web/` on disk (rust-embed debug mode); a
`--release` build embeds them into the single executable.

Test coverage includes money/rounding, day-total math, report aggregation,
CSV/HTML/PDF output, asset integrity, REST CRUD/validation, auth/CSRF/session
expiry, sequence monotonicity under concurrent pushes, push/pull pagination,
retries, conflicts/force, duplicate constraints, tombstones, and reconciliation
between employee-, day-, type- and grand totals. JavaScript tests cover Indian
money formatting, IST dates, day totals, and sync merge decisions.

Project layout:

```
src/
  main.rs            CLI, server startup, graceful shutdown
  lib.rs             router, shared state, security headers
  auth.rs            passwords, sessions, throttle, middleware
  cert.rs            local CA + server certificate generation
  db.rs              pool, migrations, sequence counter, cleanup task
  api/               HTTP handlers (auth, employees, earning types, entries, sync, reports)
  domain/            pure money, date and day-total logic
  repo/              SQL row access, earning types, pure sync decisions
  reports/           summary shaping, CSV, printable HTML, PDF
  web.rs             embedded PWA serving
web/                 no-build ES-module PWA, manifest, service worker, icons
migrations/          SQLite schema
tests/               Rust integration tests + JS logic tests
docs/                mobile install, sync protocol, operations
```

---

## Limits / non-goals

- One admin login; employees cannot sign in to view their own pay.
- No loan/advance balance ledger, tax withholding, payslips, attendance, or
  payroll-period closing. Advances and deductions are per-day amounts.
- No time tracking or hourly/day rates: earnings are recorded as business rows
  per admin-managed type.
- One server process per database. Writes are serialised by an in-process gate;
  do not point two `daybook` processes at the same file.
- Offline users can record day sheets and manage employees, but cannot change
  earning types or generate reports until reconnected.
- Offline access relies on the last successful sign-in remembered by the
  device; if the server session has expired, the device is signed out the next
  time it reaches the server.
- No push notifications, email or SMS.
- The local CA is only as trustworthy as the phones it is installed on.

---

## Troubleshooting

| Symptom | Fix |
| --- | --- |
| “Install”/offline mode unavailable on a phone | You are on HTTP. Use the TLS setup in [docs/mobile-install.md](docs/mobile-install.md), verify the padlock, then reload. |
| Certificate warning on first open | Install the generated CA and, on iOS, enable it under *Certificate Trust Settings*. |
| Login says too many attempts | 5 failures per IP in 5 minutes; wait, or restart the server to clear the in-memory throttle. |
| Phone cannot reach the server | Same network/VPN? Windows Firewall inbound rule for TCP 8787? `DAYBOOK_BIND` not `127.0.0.1`? |
| Sync keeps failing | Check the Sync tab for the error/conflicts; confirm the server clock and phone clock are roughly correct. |
| PDF shows `Rs` instead of `₹` | No suitable font found; set `DAYBOOK_PDF_FONT` to a TrueType font containing U+20B9. |
| Old UI after an upgrade | Bump `VERSION` in `web/sw.js`, restart, then reload twice or use DevTools → Application → Service Workers → Update. |
| Device shows empty data | It signed out or browser storage was cleared; sign in and let it pull from cursor 0. |

---

## License

MIT. See [../LICENSE](../LICENSE).
