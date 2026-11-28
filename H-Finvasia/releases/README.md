# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.28.1.exe` | **desktop app** (double-click) | 23869440 | `5af267076ec94be448376d62750ed7aa58f3f39157c8b3b8909a14152b0f5df6` |
| `h-finvasia-cli-v26.11.28.1.exe` | headless CLI | 10588672 | `861273b781750d1ecffb3179cca336c5fa497a2e283f25e1ab0b9800b3e7633b` |

## Running the desktop app

Double-click `h-finvasia-v26.11.28.1.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.28.1.exe doctor
.\h-finvasia-cli-v26.11.28.1.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.28.1` were CLI-only (the UI did not
exist yet). From `v26.11.28.1` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
