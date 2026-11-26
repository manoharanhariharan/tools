# Published builds

Committed, ready-to-run portable executables live here so they survive across
workspaces and machines. `release/` remains the ignored per-tag audit folder
(installer, metadata, hashes); only the portable exe and its checksum are
committed.

## Current

| File | SHA-256 |
| --- | --- |
| `h-remote-v26.11.26.4.exe` | `19ac7f375f223654688a7d6bef504519feaf404ed5b934cd851426c812bfd2da` |

`BUILD.txt` records the source commit, toolchain and build time.

## Adding a build

```powershell
.\scripts\release.ps1
```

Old executables are intentionally kept until the maintainer removes them.
