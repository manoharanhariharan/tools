# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.18.exe` | **desktop app** (double-click) | 23859200 | `54976e5ec148af7bc399de0f93dc1874814c48b9dfb98c47fd0e9d936a92f3ff` |
| `h-finvasia-cli-v26.11.27.18.exe` | headless CLI | 10588672 | `dfb93229c0cbfcdae04c1c5edda3cc8396bd7292f1b0e8c73a0bab09fa22725e` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.18.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.18.exe doctor
.\h-finvasia-cli-v26.11.27.18.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.18` were CLI-only (the UI did not
exist yet). From `v26.11.27.18` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
