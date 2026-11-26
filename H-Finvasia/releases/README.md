# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.18.exe` | **desktop app** (double-click) | 23100928 | `4e94bc5d60b3132b91f995d8873426d2b39b6f80f04821db34af176c687214b4` |
| `h-finvasia-cli-v26.11.26.18.exe` | headless CLI | 10588672 | `b1593bbfe413c39cfbfac9be10f69c9979ec8c31797dbc2c1bd5eb3162ffee60` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.18.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.18.exe doctor
.\h-finvasia-cli-v26.11.26.18.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.18` were CLI-only (the UI did not
exist yet). From `v26.11.26.18` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
