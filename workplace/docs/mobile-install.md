# Installing Daybook on Android and iPhone

No store, no APK, no signing account. The Rust server serves a PWA that installs
to the home screen and works offline.

## 1. Before you start

1. Find the server's LAN address:

   ```powershell
   ipconfig | Select-String "IPv4"
   ```

2. Allow inbound TCP **8787** through the firewall (admin PowerShell):

   ```powershell
   New-NetFirewallRule -DisplayName "Daybook (TCP 8787)" -Direction Inbound `
     -Protocol TCP -LocalPort 8787 -Action Allow -Profile Private
   ```

3. Make sure the phone is on the same Wi-Fi (or the same tailnet).

## 2. Enable HTTPS (required for install + offline)

Service workers and “Add to Home Screen” with offline caching are only allowed on
secure origins. Plain `http://192.168.x.x` runs the app online but cannot cache it.

```powershell
# Include every address the phone will use (LAN IP and/or DNS name).
.\target\release\daybook.exe cert init --host 192.168.1.20,daybook.local,localhost --out certs

$env:DAYBOOK_TLS_CERT = "certs\daybook-cert.pem"
$env:DAYBOOK_TLS_KEY  = "certs\daybook-key.pem"
$env:DAYBOOK_BIND     = "0.0.0.0:8787"
.\target\release\daybook.exe serve
```

`cert init` refuses to overwrite an existing certificate set. If you need a new
certificate (changed IP, expired, or lost phone), add `--force`; every phone must
then trust the newly generated CA.

On Linux/macOS:

```bash
./daybook cert init --host 192.168.1.20,localhost --out certs
DAYBOOK_TLS_CERT=certs/daybook-cert.pem \
DAYBOOK_TLS_KEY=certs/daybook-key.pem \
./daybook serve
```

The command creates:

| File | Purpose | Share with phones? |
| --- | --- | --- |
| `certs/daybook-ca.crt` | Local root CA | **Yes** — install this |
| `certs/daybook-cert.pem` | Server certificate chain | No |
| `certs/daybook-key.pem` | Server private key | **Never** |

The CA private key is generated in memory and never written to disk. The server
certificate is valid for 397 days (Apple's limit); re-run `cert init` before it
expires if browsers warn.

## 3. Trust the CA on each phone

### Android

1. Copy `daybook-ca.crt` to the phone (USB, Drive, email attachment — any way).
2. Open **Settings → Security (or Privacy) → More security settings →
   Encryption & credentials → Install a certificate → CA certificate**.
   Paths vary slightly by vendor; search “CA certificate” in Settings if needed.
3. Accept the warning and choose the `.crt` file.
4. Open Chrome and go to `https://192.168.1.20:8787`. The padlock must be normal
   (no warning). If Chrome still warns, the CA was installed as a user
   certificate but not trusted for apps; re-check the install path.

### iPhone / iPad

1. Send `daybook-ca.crt` to the phone (AirDrop, Files app, or email).
2. Tap the file → **Profile Downloaded** appears in Settings.
3. **Settings → General → VPN & Device Management → Daybook Local CA → Install**.
4. **Settings → General → About → Certificate Trust Settings** → enable
   **Full Trust** for *Daybook Local CA*.
5. Open Safari and go to `https://192.168.1.20:8787`; the address bar must show a
   normal padlock.

To remove access later: delete the profile under **VPN & Device Management**
(and disable trust first). Generate a new CA if a phone is lost.

## 4. Install the app

### Android (Chrome)

Open the URL, sign in, then **⋮ menu → Add to Home screen / Install app**.
Chrome may show an install prompt on its own once the service worker is active.
The app opens without browser chrome and keeps working with no signal.

### iPhone (Safari)

Open the URL, sign in, tap **Share → Add to Home Screen → Add**. iOS only offers
this from Safari, not Chrome. The installed app uses a standalone window and its
own storage.

## 5. Using it on the shop floor

- Today → tap an employee → enter the amount for each earning type (Sales,
  Service, …), an optional description, plus bonus/advance/deduction → Save.
  The day appears immediately, marked **Pending** until the server confirms it.
- Earning types are managed in the Employees tab (rename, deactivate, add,
  delete). They are cached on the phone for offline entry.
- The status chip in the header shows **Offline**, **N pending**, **Syncing…**,
  **N conflicts**, or **Up to date**.
- Offline entries stay on the device and sync automatically when the phone
  reconnects, when the app becomes visible, or every 60 seconds.
- If two devices changed the same record, the Sync tab shows both versions and
  asks you to choose. Nothing is discarded silently.
- Reports and PDF/CSV downloads need a connection (the server computes them).
- Soft-deleted employees disappear from the entry screen but their past pay
  remains in reports.

## 6. Remote access with Tailscale (alternative to the local CA)

If phones need to reach the server from outside the shop:

1. Install Tailscale on the server and each phone and sign both into the same
   tailnet.
2. Use **Tailscale Serve** to expose the local Daybook port over HTTPS, e.g.:

   ```bash
   tailscale serve --bg 8787
   ```

   This gives a trusted certificate for `https://<machine>.<tailnet>.ts.net`, so
   no CA installation is needed.
3. Open that HTTPS URL on the phone and install the PWA as above.
4. Restrict tailnet ACLs so only the owner's devices can reach the server, and
   keep the admin password strong.

Do not expose port 8787 directly to the public internet.

## 7. Updating and removing

- **Update:** replace the server binary, bump `VERSION` in `web/sw.js`, restart.
  Installed apps show a “new version is ready” prompt; reload.
- **Remove a phone:** delete the home-screen app (clears its IndexedDB copy) and,
  if desired, remove the CA profile. Revoke server sessions by resetting the
  admin password: `daybook admin set-password`.
