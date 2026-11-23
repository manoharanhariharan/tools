# Feature traceability — Phase 1

Status: **Done**, **Infra** (consumer in a later phase), **Deferred**.

| ID | Status | Implementation | Verification |
|---|---|---|---|
| P1-01 | Done | `hr-domain/src/protocol.rs`, `server.rs`, `icon.rs`, registry | base/icon/validation tests; registry tests |
| P1-02 | Done | `AddressPortBase`, alternate credentials | address/subtitle/alternate tests |
| P1-03 | Done | `UserPasswordBase`, inheritance | exclusivity/inheritance tests |
| P1-04 | Done | `hr-domain/src/registry.rs` | registry tests |
| P1-05 | Done | `RdpServer` (all appendix fields) | JSON round trip; validation tests |
| P1-06 | Done | `RemoteAppServer` | round trip |
| P1-07 | Done | `VncServer` | round trip |
| P1-08 | Done | `SshServer` | round trip |
| P1-09 | Done | `TelnetServer` | round trip |
| P1-10 | Done | `SerialServer` | round trip/validation |
| P1-11 | Done | `SftpServer` | round trip |
| P1-12 | Done | `FtpServer` | round trip |
| P1-13 | Done | `LocalAppServer` + `local_app` | round trip/macro/template tests |
| P1-14 | Done | `DummyServer` | round trip |
| P1-15 | Done | `ProtocolKind::builtin_ids`, custom validation | validation test |
| P1-16 | Done | `credential.rs` | hash/duplicate tests |
| P1-17 | Done | `alternate_credentials` | alternate tests |
| P1-18 | Done | `resolve_effective` / `effective_credentials` | inheritance tests |
| P1-19 | Done | `is_editable` + repository guards | read-only credential test |
| P1-20 | Done | `hr-platform::network::tcp_reachable` | reachability tests |
| P1-21 | Done | `tag.rs` | tag tests |
| P1-22 | Done | `normalize_tags` | normalisation tests |
| P1-23 | Done | `folder.rs`, tag→folder migration | folder/migration tests |
| P1-24 | Done | `rename_tag`, `remove_tag`, `collect_tags`; locality pins | cascade tests |
| P1-25 | Done | `local_app::AppArgument` | argument tests |
| P1-26 | Done | `expand_macros` | macro tests |
| P1-27 | Done | `templates()` | template tests |
| P1-28 | Done | schema `Servers` | storage tests |
| P1-29 | Done | schema `Credentials` | storage tests |
| P1-30 | Done | schema `Configs` + `EncryptionTest` | config/sentinel tests |
| P1-31 | Done | `TableMeta` stamps | cache/stamp tests |
| P1-32 | Done | SQLx repository + dialect builders | dialect/repository tests |
| P1-33 | Done | `Database` open/writable/read-only + migrations | database tests |
| P1-34 | Done | repository CRUD (single/batch) | CRUD/bulk tests |
| P1-35 | Done | `list_servers_page` | pagination test |
| P1-36 | Done | Phase 0 installation salt | salt tests (Phase 0) |
| P1-37 | Done | `codec.rs` seal/unseal | codec/storage tests |
| P1-38 | Done | sentinel + `AppError::EncryptionKey` | wrong-key test |
| P1-39 | Done | keyring/vault from Phase 0 | crypto tests |
| P1-40 | Done | `VaultCodec`, passphrase fallback | crypto tests |
| P1-41 | Done | redacted secrets, raw DB assertions | codec/storage tests |
| P1-42 | Done | table stamps + `TableMeta` | external stamp test |
| P1-43 | Done | shared snapshot caches + invalidation | cache tests |
| P1-44 | Infra (Phase 2) | `Arc<Vec<_>>` snapshots + paginated commands | pagination test; UI batching arrives with catalog |
| P1-45 | Done (bulk) | single transaction, encrypt/validate once, invalidate once | bulk atomicity test; no property notifications in Rust |

## Verification commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
npm run check && npm test
npm run test:e2e
npm run check:i18n
npm run check:traceability
```

`check:traceability` validates both Phase 0 and Phase 1 IDs.
