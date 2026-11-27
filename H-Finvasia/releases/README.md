# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.12.exe` | **desktop app** (double-click) | 23622144 | `f3b6a04029aace167a8326dec95c312476ef6dcb99bbeeece6efbcc74ee89368` |
| `h-finvasia-cli-v26.11.27.12.exe` | headless CLI | 10588672 | `857f65367cf617bc30280ba85d9adf59bc6c1e0bdb440bc9582537b2b39f4dda` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.12.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.12.exe doctor
.\h-finvasia-cli-v26.11.27.12.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.12` were CLI-only (the UI did not
exist yet). From `v26.11.27.12` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
