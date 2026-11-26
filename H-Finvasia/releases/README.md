# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.13.exe` | **desktop app** (double-click) | 22711808 | `e3666ced468e14a0452902764fd029a0b753f74f0897d2d3824b0a5dd6866638` |
| `h-finvasia-cli-v26.11.26.13.exe` | headless CLI | 10588672 | `98d5eb128ba95f00626cf52d04775fb11c0c9358b5ed717fbb479397a126f3b3` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.13.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.13.exe doctor
.\h-finvasia-cli-v26.11.26.13.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.13` were CLI-only (the UI did not
exist yet). From `v26.11.26.13` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
