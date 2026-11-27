# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.8.exe` | **desktop app** (double-click) | 23610880 | `6b5aecc8a2fa85a1c08f105b49a222a7b401de8d6adc191b7e4b4dc532315079` |
| `h-finvasia-cli-v26.11.27.8.exe` | headless CLI | 10588672 | `9e8bcdcb0b7bc464f54d0060868c78e92fccb172f94f78cc5986d5642d957c01` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.8.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.8.exe doctor
.\h-finvasia-cli-v26.11.27.8.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.8` were CLI-only (the UI did not
exist yet). From `v26.11.27.8` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
