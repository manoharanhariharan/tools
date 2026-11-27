# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.3.exe` | **desktop app** (double-click) | 23578624 | `5d547f5811a7eeddc9d29789ea63711fa16b11daea8a85b58b758a2b071a82b7` |
| `h-finvasia-cli-v26.11.27.3.exe` | headless CLI | 10588672 | `e5ac0ca254112987314ce3a41dc50d7c693dd627a382811ff3735611ee2144e0` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.3.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.3.exe doctor
.\h-finvasia-cli-v26.11.27.3.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.3` were CLI-only (the UI did not
exist yet). From `v26.11.27.3` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
