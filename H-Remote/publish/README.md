# Published builds

Committed, ready-to-run portable executables live here so they survive across
workspaces and machines. `release/` remains the ignored per-tag audit folder
(installer, metadata, hashes); only the portable exe and its checksum are
committed.

## Current

| File | SHA-256 |
| --- | --- |
| `h-remote-v26.11.25.4.exe` | `f33d2f5f4cd12255bd88eec598e116888185ac651d742df26fbcc4c88af77ea8` |

`BUILD.txt` records the source commit, toolchain and build time.

## Adding a build

```powershell
.\scripts\release.ps1
```

Old executables are intentionally kept until the maintainer removes them.
