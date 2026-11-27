# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.4.exe` | **desktop app** (double-click) | 23578624 | `398e483fbc7e640816fc7c45b722e72a7019b2c8d5b501f63a7aa8d90485ccc8` |
| `h-finvasia-cli-v26.11.27.4.exe` | headless CLI | 10588672 | `3df7d1d36567b751cbe83e4c66211d701bf769ce2ce22509ccbbb08fae351065` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.4.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.4.exe doctor
.\h-finvasia-cli-v26.11.27.4.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.4` were CLI-only (the UI did not
exist yet). From `v26.11.27.4` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
