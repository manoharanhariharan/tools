# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.27.9.exe` | **desktop app** (double-click) | 23610880 | `a6df1e8d54f870e729f4e8ba38ad9e45a4130c3b1ae11276e42e1599738f8846` |
| `h-finvasia-cli-v26.11.27.9.exe` | headless CLI | 10588672 | `bd5c66d1b0b194bfe3ade0799623ae6981ff5cf0da4e8eabe06ec22fb96a5ef7` |

## Running the desktop app

Double-click `h-finvasia-v26.11.27.9.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. It restores the last watch list and connects live when a valid
stored session exists.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.27.9.exe doctor
.\h-finvasia-cli-v26.11.27.9.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.27.9` were CLI-only (the UI did not
exist yet). From `v26.11.27.9` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
