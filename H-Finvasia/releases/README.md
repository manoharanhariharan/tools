# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.5.exe` | **desktop app** (double-click) | 23609856 | `a32ad60130f490aab0a75ffd978797819fb1b438938f14a340c5745f2aab668e` |
| `h-finvasia-cli-v26.11.27.5.exe` | headless CLI | 10588672 | `380f3564c564517f930c35420ee15d2ad385e97e2978d64c0c5f8a99f4bc9da6` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.5.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.5.exe doctor
.\h-finvasia-cli-v26.11.27.5.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.5` were CLI-only (the UI did not
exist yet). From `v26.11.27.5` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
