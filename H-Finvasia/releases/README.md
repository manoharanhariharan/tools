# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.17.exe` | **desktop app** (double-click) | 23785472 | `77042e7baba36baea89c111cc501f98deb0005ef2b6e4fa640080fddb9d3ec54` |
| `h-finvasia-cli-v26.11.27.17.exe` | headless CLI | 10588672 | `b5bc7c9606f90d32c46eddb0ed07369cbbd7a936fb1b198ebd192c5a4541b701` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.17.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.17.exe doctor
.\h-finvasia-cli-v26.11.27.17.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.17` were CLI-only (the UI did not
exist yet). From `v26.11.27.17` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
