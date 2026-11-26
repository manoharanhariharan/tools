# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.22.exe` | **desktop app** (double-click) | 23319040 | `33f8cd58249224bb9bf3d0f0b895605923cbae945e6387858c940659053a3c03` |
| `h-finvasia-cli-v26.11.26.22.exe` | headless CLI | 10588672 | `b1593bbfe413c39cfbfac9be10f69c9979ec8c31797dbc2c1bd5eb3162ffee60` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.22.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.22.exe doctor
.\h-finvasia-cli-v26.11.26.22.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.22` were CLI-only (the UI did not
exist yet). From `v26.11.26.22` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
