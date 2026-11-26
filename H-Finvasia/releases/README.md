# Released builds

Committed, ready-to-run portable executables for H-Finvasia. `BUILD.txt` records
the source commit, toolchain and build time for the current build, and
`SHA256SUMS.txt` covers every executable in this folder.

## Current

| File | Size | SHA-256 |
| --- | ---: | --- |
| `h-finvasia-v26.11.26.1.exe` | 10550784 | `99b4cd38a72479c4c926f6e1daca677f1e4d6749d46a252b17174ed552b18d27` |

## Running

```powershell
.\h-finvasia-v26.11.26.1.exe doctor
.\h-finvasia-v26.11.26.1.exe master stats
```

Requires a local MongoDB (`FV_MONGO_URI`, default `mongodb://127.0.0.1:27017`)
for the `master` commands; `doctor`, `replay` and `bench` work without it.

## Adding a build

```powershell
.\scripts\release.ps1
```

Old executables are intentionally kept until the maintainer removes them.
