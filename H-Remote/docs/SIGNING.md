# Code signing and SmartScreen

Unsigned Windows builds trigger SmartScreen/Defender warnings, especially for a
freshly downloaded portable exe. This project builds unsigned by default; signing is
opt-in and never required for local development.

## Local signing

`scripts/release.ps1` signs every `.exe`/`.msi` in the tag folder when
`HREMOTE_SIGN_CERT` is set:

```powershell
$env:HREMOTE_SIGN_CERT = "C:\certs\h-remote-codesign.pfx"
$env:HREMOTE_SIGN_PASSWORD = "<password>"     # or use a hardware token/CI secret
$env:HREMOTE_TIMESTAMP_URL = "http://timestamp.digicert.com"   # optional
.\scripts\release.ps1 -Bundles all
```

The script runs `signtool sign /fd SHA256 /tr <url> /td SHA256` and then
`signtool verify /pa`. The result is recorded as `signed: yes|no` in `BUILD.txt`.

Without signtool the script fails rather than silently producing unsigned
artifacts when a certificate was requested.

## CI signing

`.github/workflows/release.yml` signs Windows artifacts when these repository
secrets exist:

| Secret | Meaning |
|---|---|
| `WINDOWS_CERT_PFX_BASE64` | Base64-encoded `.pfx` (export with `certutil -encode`) |
| `WINDOWS_CERT_PASSWORD` | PFX password |

When absent, the job builds unsigned and `BUILD.txt` reports `signed: unknown`.
Secrets are never echoed; keep the PFX in a secret store, not the repo.

## SmartScreen expectations

- **Unsigned**: SmartScreen may show “Unknown publisher”. Users can keep the file;
  reputation builds only with consistent signing and downloads.
- **Standard OV certificate**: reputation accrues per certificate and download
  pattern; early releases still warn.
- **EV certificate or Azure Trusted Signing**: immediate reputation in most cases;
  requires a hardware token or cloud signing service and is recommended before a
  broad release.
- **Timestamping** keeps signatures valid after the certificate expires; always
  pass `/tr`/`/td`.
- **Reproducibility**: publish `BUILD.txt` and `SHA256SUMS.txt` next to the
  download so users can verify the hash independently of the signature.

## Verifying a download

```powershell
Get-FileHash .\h-remote-v26.11.22.1.exe -Algorithm SHA256
Get-AuthenticodeSignature .\h-remote-v26.11.22.1.exe
```

Compare the hash with `SHA256SUMS.txt` from the same release tag.
