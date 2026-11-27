# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.7.exe` | **desktop app** (double-click) | 23606784 | `236586a3f5a0366ee9f0d4b9b0fb401f5719a0c4b7d205845ebab8520fd82201` |
| `h-finvasia-cli-v26.11.27.7.exe` | headless CLI | 10588672 | `534754a92bc3feeb9060e25dfd87c6292a77ea9bbe03dfd5bb0ffb527787d186` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.7.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.7.exe doctor
.\h-finvasia-cli-v26.11.27.7.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.7` were CLI-only (the UI did not
exist yet). From `v26.11.27.7` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
