# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.16.exe` | **desktop app** (double-click) | 23785472 | `99067df622e65f79b12bf50f1b2d5d0c4150a2064041d12496502337b79fc7af` |
| `h-finvasia-cli-v26.11.27.16.exe` | headless CLI | 10588672 | `c72fe982b8eb5cb63a12bb0e2d6c1ab82ca8a7771b61b4871c3c9716d59c0593` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.16.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.16.exe doctor
.\h-finvasia-cli-v26.11.27.16.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.16` were CLI-only (the UI did not
exist yet). From `v26.11.27.16` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
