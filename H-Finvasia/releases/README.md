# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.28.2.exe` | **desktop app** (double-click) | 23912448 | `0154b1e877fe32ca8413fc9735a7ac2a4849be0f88a41612fba3486e60e3ebb4` |
| `h-finvasia-cli-v26.11.28.2.exe` | headless CLI | 10588672 | `ca95e014a8e88c4ef660051b00b29ce9677ae1f61f39d91152c1ab9c942e798a` |

## Running the desktop app

Double-click `h-finvasia-v26.11.28.2.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.28.2.exe doctor
.\h-finvasia-cli-v26.11.28.2.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.28.2` were CLI-only (the UI did not
exist yet). From `v26.11.28.2` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
