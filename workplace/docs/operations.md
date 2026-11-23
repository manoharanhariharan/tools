# Running Daybook day to day

## Data and ports

| Item | Default |
| --- | --- |
| Listen address | `0.0.0.0:8787` |
| Database | `<data-dir>/daybook.db` (+ `-wal`, `-shm`) |
| Windows data dir | `%LOCALAPPDATA%\Daybook` |
| Linux data dir | `$XDG_DATA_HOME/daybook` or `~/.local/share/daybook` |
| Health endpoint | `GET /api/health` (no auth) |

Always set an explicit `DAYBOOK_DATA_DIR` for a service account, because
`LOCALAPPDATA` differs per user (and is not what you expect under `SYSTEM`).

## Windows: run as a background task

The simplest reliable option is Task Scheduler (no third-party service wrapper).

1. Create `C:\Daybook` (binary, `web` is embedded, `certs\`, `data\`).
2. Open **Task Scheduler → Create Task**:
   - **General:** Run whether user is logged on or not; run with highest
     privileges; configure for Windows 10/11.
   - **Triggers:** At startup, delay 30 seconds.
   - **Actions:** Start a program:
     - Program: `C:\Daybook\daybook.exe`
     - Arguments: `serve`
     - Start in: `C:\Daybook`
   - **Settings:** Restart on failure every 1 minute, 3 attempts; do not stop on
     battery/idle.
3. Set machine-wide environment variables (admin PowerShell) and reboot:

   ```powershell
   [Environment]::SetEnvironmentVariable("DAYBOOK_DATA_DIR", "C:\Daybook\data", "Machine")
   [Environment]::SetEnvironmentVariable("DAYBOOK_BIND", "0.0.0.0:8787", "Machine")
   [Environment]::SetEnvironmentVariable("DAYBOOK_TLS_CERT", "C:\Daybook\certs\daybook-cert.pem", "Machine")
   [Environment]::SetEnvironmentVariable("DAYBOOK_TLS_KEY", "C:\Daybook\certs\daybook-key.pem", "Machine")
   ```

4. Allow the port (private profile) and check `http(s)://<host>:8787/api/health`.

Logs go to stdout/stderr. With Task Scheduler they are not captured by default;
either run under `cmd /c "daybook.exe serve >> C:\Daybook\daybook.log 2>&1"`, or
wrap with NSSM if you prefer a real Windows service with log files.

> Bind to `0.0.0.0` only on a trusted network. Use Tailscale for remote access
> instead of exposing the port to the internet.

## Linux: systemd

`/etc/systemd/system/daybook.service`:

```ini
[Unit]
Description=Daybook earnings
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=daybook
Group=daybook
WorkingDirectory=/opt/daybook
Environment=DAYBOOK_DATA_DIR=/var/lib/daybook
Environment=DAYBOOK_BIND=0.0.0.0:8787
Environment=DAYBOOK_TLS_CERT=/opt/daybook/certs/daybook-cert.pem
Environment=DAYBOOK_TLS_KEY=/opt/daybook/certs/daybook-key.pem
Environment=RUST_LOG=daybook=info
ExecStart=/opt/daybook/daybook serve
Restart=on-failure
RestartSec=5

# Hardening (adjust for your paths)
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=/var/lib/daybook

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now daybook
journalctl -u daybook -f
```

On Linux, PDF fonts are discovered from DejaVu/Liberation/Noto; install
`fonts-dejavu-core` if no font is found, or set `DAYBOOK_PDF_FONT`.

## Health checks

`GET /api/health` returns `{"status":"ok", ...}` only after a real `SELECT 1`.
Use it for uptime monitoring. It is intentionally unauthenticated and leaks no
business data.

## Backups

Preferred (online, consistent):

```bash
sqlite3 /var/lib/daybook/daybook.db ".backup '/backups/daybook-$(date +%F).db'"
```

Retain at least a few days. Test a restore occasionally.

Restore procedure:

1. Stop the service.
2. Move the current `daybook.db`, `daybook.db-wal`, `daybook.db-shm` aside.
3. Put the backup in place as `daybook.db` (no `-wal`/`-shm`).
4. Start the service and check `/api/health`.

Phones resume from their cursor. If the restored database is older than a
phone's cursor, the PWA detects the sequence rollback and rebuilds its local
non-pending rows from scratch; queued offline edits are pushed afterwards.

## Upgrades

1. Back up first.
2. Build or copy the new `daybook` binary.
3. Bump `VERSION` in `web/sw.js` (e.g. `daybook-v2`).
4. Replace the binary, restart the service.
5. Migrations run at startup; startup fails loudly if a migration cannot apply.
6. Installed PWAs show a reload prompt when the new service worker activates.
   Devices with the app closed update on their next launch.

If the server IP changed, regenerate certificates with `--force` and re-install
the new CA on each phone (or move to Tailscale HTTPS, which avoids the CA).

Never run two server processes against the same database.

## Resource behaviour

- SQLite in WAL mode, one writer at a time through an in-process async gate;
  readers never block writers.
- Connection pool: 5 connections, 10 s acquire timeout, 5 s busy timeout.
- Expired sessions are deleted hourly; the login throttle map is capped at 4,096
  IPs.
- Pull pages are capped at 200 rows / 500 max, push batches at 200 changes.
- Reports are limited to 366 days per request.
- The PDF font is loaded once into memory (~1 MB) and reused; PDF rendering runs
  on a blocking thread so it cannot stall request handling.
- The browser keeps one IndexedDB connection, aborts view listeners on
  navigation, clears its sync interval on logout, bounds toasts to four, and
  revokes download object URLs.

## Monitoring and diagnosing

- `daybook doctor` prints the data directory, whether TLS is configured, the
  chosen PDF font, and row/sequence counts.
- Sync issues are visible in the app's **Sync** tab with both versions.
- If clients cannot log in, check the server clock (TLS/cookie validity), the
  session lifetime, and the login throttle.
- If the UI behaves oddly after an upgrade, bump the service worker version and
  reload; hard-refresh if needed.
