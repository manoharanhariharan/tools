# Published builds

Committed, ready-to-run portable executables live here so they survive across
workspaces and machines. `release/` remains the ignored per-tag audit folder
(installer, metadata, hashes); only the portable exe and its checksum are
committed.

## Current

| File | SHA-256 |
| --- | --- |
| `h-remote-v26.11.23.8.exe` | `3a725a40011722e0d33c9f85341dc9f9920c6691b36011182b7680feabe83bfd` |

`BUILD.txt` records the source commit, toolchain and build time.

## Adding a build

```powershell
.\scripts\release.ps1
Copy-Item .\release\v<tag>\h-remote-v<tag>.exe .\publish\
(Get-FileHash .\publish\h-remote-v<tag>.exe -Algorithm SHA256).Hash.ToLower() +
  "  h-remote-v<tag>.exe" | Set-Content .\publish\SHA256SUMS.txt
git add publish
git commit -m "release(h-remote): publish v<tag>"
git push
```

Old executables are intentionally kept until the maintainer removes them.
