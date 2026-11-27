# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.1.exe` | **desktop app** (double-click) | 23578624 | `56e5f9ca56e3103ff6f89ff3d2a54184782484e6caed50317c0dad2915e160aa` |
| `h-finvasia-cli-v26.11.27.1.exe` | headless CLI | 10588672 | `9179946aaa669a4b682858bd1618f0eb991ddee21b2f52649dcb13a763245501` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.1.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.1.exe doctor
.\h-finvasia-cli-v26.11.27.1.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.1` were CLI-only (the UI did not
exist yet). From `v26.11.27.1` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
