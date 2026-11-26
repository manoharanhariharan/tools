# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.12.exe` | **desktop app** (double-click) | 22718464 | `d9e9387142734f342604292bbab69345a7ea10c12d4d128fbfc8532628e1df62` |
| `h-finvasia-cli-v26.11.26.12.exe` | headless CLI | 10586624 | `d1e05ab3e596a8d85d8f594e3f227e4c11528faec6308ccf0a269e4ae3343579` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.12.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.12.exe doctor
.\h-finvasia-cli-v26.11.26.12.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.12` were CLI-only (the UI did not
exist yet). From `v26.11.26.12` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
