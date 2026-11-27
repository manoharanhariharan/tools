# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.13.exe` | **desktop app** (double-click) | 23625728 | `90fe5c138ba545d49fce353f7062d52289e6f5fc96779ec018b0893301d3ebe7` |
| `h-finvasia-cli-v26.11.27.13.exe` | headless CLI | 10588672 | `a00dd553de7b501ee7a68fac90502b60ab7fef326a4bbdc935260f297cdb107d` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.13.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.13.exe doctor
.\h-finvasia-cli-v26.11.27.13.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.13` were CLI-only (the UI did not
exist yet). From `v26.11.27.13` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
