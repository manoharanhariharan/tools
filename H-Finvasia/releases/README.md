# Released builds

Committed, ready-to-run builds for H-Finvasia. `BUILD.txt` records the source
commit, toolchain and build time for the current build, and `SHA256SUMS.txt`
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| `h-finvasia-v26.11.26.9.exe` | **desktop app** (double-click) | 22657536 | `456b677c90df8a792d15460fd13ba0b3e37bd30ea0b1fcebf8a226b92a240b61` |
| `h-finvasia-cli-v26.11.26.9.exe` | headless CLI | 10586624 | `70d7e226dba86945be36fcd0d06cdf79522ba9b825bc390cd05fba3941a16973` |

## Running the desktop app

Double-click `h-finvasia-v26.11.26.9.exe`. It loads the instrument master from the local
MongoDB (`STOCK.FINVASIA_SYMBOL`) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

```powershell
.\h-finvasia-cli-v26.11.26.9.exe doctor
.\h-finvasia-cli-v26.11.26.9.exe master stats
```

`master` needs a local MongoDB (`FV_MONGO_URI`, default
`mongodb://127.0.0.1:27017`); `doctor`, `replay` and `bench` do not.

## A note on older files

`h-finvasia-v*.exe` builds **before** `v26.11.26.9` were CLI-only (the UI did not
exist yet). From `v26.11.26.9` the unqualified name is the desktop app and the CLI
is suffixed `-cli`.

## Adding a build

```powershell
.\scripts\release.ps1
```
