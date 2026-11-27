# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.11.exe` | **desktop app** (double-click) | 23611904 | `6299d9235c8e6b8c4df0fa9fbc0dea2b53bb047b850487b6475bfb165a6fae77` |
| `h-finvasia-cli-v26.11.27.11.exe` | headless CLI | 10588672 | `2916d90118726eb1eaea193d5642ef902146c4cbb29c11d25e8ecbb26f308be5` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.11.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.11.exe doctor
.\h-finvasia-cli-v26.11.27.11.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.11` were CLI-only (the UI did not
exist yet). From `v26.11.27.11` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
