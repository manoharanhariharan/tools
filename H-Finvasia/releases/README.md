# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.14.exe` | **desktop app** (double-click) | 23769600 | `60e9bb8eaedf22764e4f69ee2eb44c851e22e3e4424010c1fff8281eae4653be` |
| `h-finvasia-cli-v26.11.27.14.exe` | headless CLI | 10588672 | `921cb2a97c509dcd7b78f97b6f1b8885c0df7cd8dc9acb069242409f59af0fb5` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.14.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.14.exe doctor
.\h-finvasia-cli-v26.11.27.14.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.14` were CLI-only (the UI did not
exist yet). From `v26.11.27.14` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
