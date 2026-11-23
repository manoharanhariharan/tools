# Vendored `ironrdp-rdpsnd-native`

- Upstream: <https://github.com/Devolutions/IronRDP>
- Version: `0.7.0` (crates.io)
- License: MIT OR Apache-2.0 (same as upstream)

## Why this copy exists

Upstream enables the `opus` feature by default, which builds bundled `libopus`
through CMake (`libopus_sys`). H-Remote must build on machines and CI images
without CMake/nasm, so this copy sets:

```toml
default = []
```

`src/cpal.rs` still contains the Opus path behind `#[cfg(feature = "opus")]`;
re-enable the feature (and remove the `[patch.crates-io]` entry in the workspace
`Cargo.toml`) if the toolchain becomes available.

Only two `#[allow(unused_mut)]` annotations were added so the PCM-only build is
warning-free. No behavior was changed otherwise.

When updating IronRDP, re-copy `src/`, `examples/`, `README.md` and
`Cargo.toml`, then re-apply the `default = []` change.
