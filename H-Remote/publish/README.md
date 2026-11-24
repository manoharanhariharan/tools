# Published builds

Committed, ready-to-run portable executables live here so they survive across
workspaces and machines. `release/` remains the ignored per-tag audit folder
(installer, metadata, hashes); only the portable exe and its checksum are
committed.

## Current

| File | SHA-256 |
| --- | --- |
| `h-remote-v26.11.24.3.exe` | `d31b50220f5fb5ceec1bfd860662a0b688ba679a3b7e0e9b3cf71158a9f1bdfb` |

`BUILD.txt` records the source commit, toolchain and build time.

## Adding a build

```powershell
.\scripts\release.ps1
```

Old executables are intentionally kept until the maintainer removes them.
