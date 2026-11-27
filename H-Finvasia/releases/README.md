# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.15.exe` | **desktop app** (double-click) | 23770112 | `0b20c073b148f149957c21c7e81e158b920a06a4aa95c4f8f9c63d94175d8a37` |
| `h-finvasia-cli-v26.11.27.15.exe` | headless CLI | 10588672 | `848bab9d32a5c146b0585a2eab044ef88fb6b082550d5dc56326672325dc51bc` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.15.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.15.exe doctor
.\h-finvasia-cli-v26.11.27.15.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.15` were CLI-only (the UI did not
exist yet). From `v26.11.27.15` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
