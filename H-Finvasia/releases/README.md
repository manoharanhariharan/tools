# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.3.exe` | **desktop app** (double-click) | 20360704 | `a704fb3671c3891e7e3af45980683495d666da95fc1a821aa1283aa3fef44e90` |
| `h-finvasia-cli-v26.11.26.3.exe` | headless CLI | 10550784 | `36e50275ae3f2ca385bd6988abc7834889d3cc4ec637796de7f2d610d9e6f9b2` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.3.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.3.exe doctor
.\h-finvasia-cli-v26.11.26.3.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.3` were CLI-only (the UI did not
exist yet). From `v26.11.26.3` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
