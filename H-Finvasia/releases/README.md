# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.4.exe` | **desktop app** (double-click) | 20389376 | `797bb9de51003620b1a1ca3647997f670602e0eb5978cb2454d75f595ffd3219` |
| `h-finvasia-cli-v26.11.26.4.exe` | headless CLI | 10550784 | `36e50275ae3f2ca385bd6988abc7834889d3cc4ec637796de7f2d610d9e6f9b2` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.4.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.4.exe doctor
.\h-finvasia-cli-v26.11.26.4.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.4` were CLI-only (the UI did not
exist yet). From `v26.11.26.4` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
