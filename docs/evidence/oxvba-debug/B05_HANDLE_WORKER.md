# B05 Handle Worker Evidence

Bead: `bd-00fz.6`  
Scope: `DebugSessionHandle` worker skeleton and sync command marshalling.

## Implementation evidence

- `attach_debug_session(Arc<Engine>, ProjectManifest, DebugAttachConfig)` spawns the worker through `spawn_debug_worker`.
- The worker thread constructs and owns `DebugSessionCore`; the handle only stores a `crossbeam_channel::Sender<DebugCommand>`, session id, and join handle guarded by `Mutex`.
- `DebugSessionHandle` is cloneable and uses no `unsafe` Send/Sync implementation.
- Public sync commands marshal through typed `DebugCommand` variants and typed one-shot replies.
- Completion projects to `DebugRunResultView::Exited` through `run_result_view_from_core`.

## Checks run

- `cargo test -p oxvba-debug --test handle_attach --test handle_start --test handle_continue --test handle_step_into --test handle_step_over --test handle_step_out --test handle_breakpoint_set --test handle_breakpoint_toggle --test handle_breakpoint_clear --test handle_breakpoints --test handle_completion --test handle_inspect --test handle_watch_add --test handle_watch_evaluate --test handle_watch_modify --test handle_watch_remove --test concurrency_serialization --test handle_send_sync`
- `cargo test -p oxvba-debug`

## Race-sensitive lane

The race-sensitive B05 lane is represented by `tests/concurrency_serialization.rs`, which drives eight cloned handles concurrently through the crossbeam command channel and verifies all commands are serialized by the worker-owned core. A ThreadSanitizer run was not available in this Windows stable-toolchain environment; no unsafe Send/Sync implementation exists in the handle path.
