# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.6.exe` | **desktop app** (double-click) | 22305280 | `a4c1b89378003e274bc368b1eb49cb401d152f96cf68d2559db2e79aaad12991` |
| `h-finvasia-cli-v26.11.26.6.exe` | headless CLI | 10554880 | `d99b3fb6f043be5b0e5822ebbd25bda499cb58e9269dbf5c5ffe8d4fabd50b40` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.6.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.6.exe doctor
.\h-finvasia-cli-v26.11.26.6.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.6` were CLI-only (the UI did not
exist yet). From `v26.11.26.6` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
