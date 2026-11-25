# Resource & memory hygiene

Memory leaks and unbounded resources are build-time defects in this project, not
backlog items. Every feature must ship with its teardown and a test (or an
explicit, documented reason it cannot be tested).

## 1. Enforced patterns

### Svelte listeners and timers

- All backend subscriptions go through
  `subscribe(event, handler, CleanupRegistry)` (`ui/src/lib/ipc/events.ts`).
  - The async `listen()` promise is guarded: if the component unmounts before it
    resolves, the unlisten function is invoked immediately.
  - `CleanupRegistry.dispose()` is idempotent, drains callbacks, and immediately
    runs callbacks registered after disposal.
- `App.svelte` creates a fresh registry per mount; no module-level registry.
- Timers/animation frames/object URLs must be cancelled in `$effect`/`onMount`
  cleanup. The catalog search debounce and the session frame poll are cancelled
  in `dispose()` and covered by store tests.
- Dialogs resolve displaced promises (`close(null)`) so callers cannot hang.

### Rust threads and handles

- `JsonStore` owns exactly one saver thread. `flush()` waits for the write;
  `Drop` sets shutdown, flushes pending state, notifies the condvar and **joins**
  the thread. A test asserts the worker's `finished` flag is true when `Drop`
  returns.
- `LogGuard` owns the non-blocking writer guard; dropping flushes log lines
  (integration-tested in a separate process).
- SQLite pools are closed explicitly on shutdown; a Windows test renames the DB
  file after `close()` to prove no handle remains (with a short retry for the
  OS release latency).
- Vault keys are `Zeroizing`; dropping the vault zeroizes the key.
- The configuration watcher is a single task per process; it ends when the store
  sender is dropped at shutdown.
- Each embedded session owns one `hremote-rdp` OS thread with a current-thread
  runtime; cancel/shutdown signals it, drops the IronRDP client and joins the
  bridge. `AppState::shutdown` calls `SessionManager::shutdown`.
- External runners/local apps spawn with `kill_on_drop`; cancel kills and awaits
  the child, and generated `.rdp`/startup files are removed after exit.
- VNC framebuffers are bounded at 4096×4096×4 bytes; out-of-bounds/truncated
  rect updates are rejected rather than growing memory.
- File transfers copy through a fixed 64 KiB buffer, check a cancellation flag
  between chunks and close the remote session on shutdown.
- The sensitive-action verification cache is a single 30-second timestamp; the
  UTF-16 password buffer from the Windows prompt is wiped best-effort after use.
- Additional sources hold at most one pooled connection each with a 5 s timeout;
  their cached snapshots stay bounded by the remote table size and are replaced
  (never appended) on refresh.
- Pre/post-connect scripts run in blocking tasks; the post script watcher is a
  single task per session with a broadcast receiver that ends at the terminal
  state or when the manager closes. Update checks are a single 60-second-looping
  watcher.
- The search cache is capped at 64 entries with a 24-hour TTL and is keyed by the
  catalog revision; large-catalog matching runs on a blocking task rather than the
  async worker.
- The launcher window is created once and hidden/shown; closing it hides rather
  than destroys it. Its resize debounce, listeners and focus timers are cleared
  on destroy.
- Global hotkeys are unregistered before every re-registration.

### Bounded work

- Search/import/transfer queues must use bounded channels. The session frame path
  is a fixed-size shared-memory ring; no unbounded `Vec<Frame>` is allowed.
- Caches must have an explicit maximum and eviction policy. Phase 1 caches are
  `Arc<Vec<_>>` snapshots keyed by table stamp; the icon cache is capped by
  entry count and total bytes. No global unbounded caches exist.
- Log rotation keeps 14 daily files.
- Crash reports and log reads are size-capped.

## 2. Session lifecycle contract (implemented)

```
SessionManager
  ├── sessions: HashMap<id, SessionSlot> (max 16)
  ├── events: bounded broadcast (256; lagged receivers skip)
  ├── slot: snapshot + StateMachine + CancelToken + JoinHandle
  └── frame transport: SharedFrameRing (single slot, seqlock) or heap
Backend
  ├── embedded: dedicated `hremote-rdp` thread + current-thread runtime
  ├── input: unbounded bridge channel to the RDP thread
  └── cancel: token + bridge shutdown; task aborted only on remove/shutdown
UI
  ├── polls visible panes at 150 ms and skips hidden tabs
  ├── stops the interval when no session is connected and on `dispose()` (tested)
  ├── one shared `sessions` window hosts every tab and the grid
  ├── main-window close exits; hidden windows never keep the process alive
  ├── shutdown awaits cancelled session tasks so child processes are reaped
  └── never retains a second copy of a frame after `frames[id]` is replaced
```

Rules:

- Every session ends in `Disconnected` or `Failed`; the manager emits the
  terminal state before the task exits.
- Cancelling a session signals cooperatively; removing/shutting down aborts the
  task to guarantee teardown.
- The frame ring is bounded at 4096×4096×4 bytes and the reader retries instead
  of blocking the producer.
- Closing a tab (Phase 4) must signal, join and reap the session; the orphan
  watchdog logs and terminates leftovers.

## 3. Review checklist (required per PR)

- [ ] Every listener/timer/rAF has a matching teardown on the same code path.
- [ ] Every spawned thread/task has an owner and a shutdown signal.
- [ ] Every file/socket/pool is closed deterministically, not just dropped late.
- [ ] Every cache/queue/channel has a bound and an eviction policy.
- [ ] Credential/server JSON caches invalidate on writes and table-stamp changes.
- [ ] No closure captures a large object graph longer than needed.
- [ ] No detached DOM element or texture reference is retained.
- [ ] A test asserts the teardown where practical (see `stores.test.ts`,
      `store::worker_thread_terminates_after_drop`,
      `closes_without_leaving_file_locks_on_windows`).
- [ ] Shutdown path: stores flushed, DB closed, vault zeroized, logs flushed.
- [ ] Additional-source pools are closed by `SourceManager::shutdown`, the source
      watcher exits on `shutdown_started`, and the settings status interval is
      cleared on destroy.
- [ ] Post-disconnect script watchers end with the session or manager shutdown;
      temporary private-key and `.rdp` files are removed on every exit path.

## 4. Verification

```bash
cargo test -p hr-config store::tests::worker_thread_terminates_after_drop
cargo test -p hr-storage closes_without_leaving_file_locks_on_windows
npm test -- stores
```

The clippy configuration denies `std::mem::forget` and direct
`Command::spawn` (use the process helper that reaps children in Phase 3).
