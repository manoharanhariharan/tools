# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.2.exe` | **desktop app** (double-click) | 23578624 | `949aa1c382b0921cab63afb93bb2d793fe4c3eb784d937bb060e380593bb5ea9` |
| `h-finvasia-cli-v26.11.27.2.exe` | headless CLI | 10588672 | `8a2013f61b1de1e73870c716ef46c9a86599383be814a6e144b662d05985deae` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.2.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.2.exe doctor
.\h-finvasia-cli-v26.11.27.2.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.2` were CLI-only (the UI did not
exist yet). From `v26.11.27.2` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
