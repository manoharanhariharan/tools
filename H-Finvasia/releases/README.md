# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.8.exe` | **desktop app** (double-click) | 22389248 | `0d386c899b802051053b6f02b6b28b8dcf8c6a4c6d0050cdce64bbe999fc6a10` |
| `h-finvasia-cli-v26.11.26.8.exe` | headless CLI | 10586624 | `17b648af724491496db44ff4a08e13a477e5bd93e0fc73e1eadc3c40cce07353` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.8.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.8.exe doctor
.\h-finvasia-cli-v26.11.26.8.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.8` were CLI-only (the UI did not
exist yet). From `v26.11.26.8` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
