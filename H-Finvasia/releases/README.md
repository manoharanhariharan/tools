# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.7.exe` | **desktop app** (double-click) | 22393344 | `a47a096cbc4182f94d3021e59b6fc77530686df4f16aa630842e15529f245288` |
| `h-finvasia-cli-v26.11.26.7.exe` | headless CLI | 10586624 | `45743781abbe426c8c5b4bd2ac0a5881f79da0ea78c33728bba617f8838f334e` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.7.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.7.exe doctor
.\h-finvasia-cli-v26.11.26.7.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.7` were CLI-only (the UI did not
exist yet). From `v26.11.26.7` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
