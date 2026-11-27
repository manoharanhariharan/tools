# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.10.exe` | **desktop app** (double-click) | 23610880 | `67e269dee28529070dc1bf725f77535b1f9926bb96bdb57f20a3de6394572f97` |
| `h-finvasia-cli-v26.11.27.10.exe` | headless CLI | 10588672 | `40f2d88900a0886f124a327bc8446ecbf17dd7adadd3bb90a4b869c487fd62f9` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.10.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.10.exe doctor
.\h-finvasia-cli-v26.11.27.10.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.10` were CLI-only (the UI did not
exist yet). From `v26.11.27.10` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
