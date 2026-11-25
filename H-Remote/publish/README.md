# Published builds

Committed, ready-to-run portable executables live here so they survive across
workspaces and machines. `release/` remains the ignored per-tag audit folder
(installer, metadata, hashes); only the portable exe and its checksum are
committed.

## Current

| File | SHA-256 |
| --- | --- |
| `h-remote-v26.11.25.7.exe` | `45d70f2fa9ded8f32cd9b13d98a0ad4f84998128255a6286e7d749ffd26d8a63` |

`BUILD.txt` records the source commit, toolchain and build time.

## Adding a build

```powershell
.\scripts\release.ps1
```

Old executables are intentionally kept until the maintainer removes them.
