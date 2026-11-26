# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.14.exe` | **desktop app** (double-click) | 22766080 | `e5770ac1ec5b7881ad54c44b83d71a8f23a6491afdf3be67f2fa4f142de02ce6` |
| `h-finvasia-cli-v26.11.26.14.exe` | headless CLI | 10588672 | `b1593bbfe413c39cfbfac9be10f69c9979ec8c31797dbc2c1bd5eb3162ffee60` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.14.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.14.exe doctor
.\h-finvasia-cli-v26.11.26.14.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.14` were CLI-only (the UI did not
exist yet). From `v26.11.26.14` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
