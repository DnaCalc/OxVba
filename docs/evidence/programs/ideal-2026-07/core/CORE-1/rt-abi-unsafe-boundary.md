# Runtime ABI unsafe-boundary and re-entry repair

Date: 2026-07-14

Bead: `bd-59co.2.2.26`

Baseline: `b5604905554d29d14277fbf17d4720184cdf12ed`

Repair commits: `a2c012bb`, `648936d7`, `b94abfda`, `60b4d88a`, and
`37796392`

Branch: `codex/bd-59co-2-2-26-rt-abi-safety`

Status: the bounded raw runtime ABI, synchronous procedure bridge, stable JIT
run-root, disjoint library-context dispatch, current VM3 queued-DoEvents
boundary, and termination-cleanup repair is implemented and passes its focused
gates and independent non-author review. Controller integration remains
required. This evidence does not claim full `RUNTIME-ABI-001`, a stable raw VM3
session root, synchronous same-VM VM3 re-entry, Windows COM behavior, product
helper-catalog completion, or workspace strict-Clippy closure.

## Resulting boundary

- All 63 exported `#[unsafe(no_mangle)]` runtime helpers are
  `pub unsafe extern "C" fn` with explicit safety contracts. Raw state,
  pointer/length, initialized-storage, aliasing, thread, and ownership
  obligations are stated at the conversion and release cores.
- `ProcInvokeBridge` and `ExecState::proc_invoker` are private. The unsafe
  install/clear ABI is the only bridge mutation surface, and the JIT clears it
  with an RAII guard before the opaque stack context expires.
- Null plus zero length maps to an empty slice for built-in invocation and
  generated UTF-8 descriptors. Integer-carried descriptor decoding exposes its
  actual unsafe storage contract.
- `JitRun` has one stable raw root for the complete session. There is no
  callback-context pointer in `JitRun` and no pointer-refresh mechanism.
  Preparation and reconciliation use short typed borrows around, never across,
  all five compiled-entry paths: top-level entry, runtime procedure callback,
  project/default-member entry, local static call, and referenced-program call.
- The same pre/entry/post split covers shared As-New operand and field access,
  direct `New` and predeclared access, event handler entry, dynamic COM dispatch,
  statement/final termination drains, object-default array dispatch, and
  built-in/DoEvents dispatch.
- Local, referenced-program, project-member, and runtime callback frames restore
  activation handler state and reconcile frame-owned maps even when compiled
  entry fails. A callee's populated `Err` remains the observable error, while
  caller `On Error`/active-handler state is restored.

## Library and host re-entry policy

`LibContext` owns RNG state only. The dispatcher now makes that ownership rule
structural:

- `invoke_context_free` covers the exhaustive catalog but rejects `Rnd` and
  `Randomize` with `ContextRequired`; it may receive the host but no mutable
  `LibContext`.
- `invoke_contextual` accepts only `Rnd` and `Randomize`; it receives mutable
  `LibContext` but no host.
- `is_contextual` is the shared classifier. A `NativeImplId::ALL` regression
  proves that its complete contextual set is exactly `Rnd` and `Randomize`;
- the generic safe `invoke(id, args, host, &mut LibContext)` API was removed.
  Such a signature cannot express that host-capable dispatch and mutable
  execution context must not overlap;
- `rt_lib_invoke_with_policy` copies the host under a short state borrow, drops
  that borrow before context-free host dispatch, and borrows RNG context only
  for the non-host contextual branch;
- VM3 now makes the same explicit classification. Contextual RNG dispatch takes
  only `&mut LibContext`; every other ID takes only the host and fails closed if
  classification ever disagrees with the exhaustive context-free dispatcher.

This removes the safe library API that permitted a host call such as `DoEvents`
while the same call also retained mutable `LibContext`. The JIT/runtime-ABI path
uses its stable raw session root and short pre/post borrows. Current VM3 has a
narrower event model: `HostServices` exposes shared HAL facets but grants no VM
or session callback authority; `StandardHostServices::DoEvents` performs a
bounded pump and marks queued COM work, returns, and VM3 polls/dispatches the
queue from the following `StmtBoundary`. A custom host can synchronously access
the same VM only by obtaining and dereferencing separate unsafe/raw authority;
the safe `HostServices`/VM3 APIs do not enable that operation.

Consequently this repair does **not** pretend that a leaf raw helper makes VM3
synchronously re-entrant. A sound future same-VM callback design must convert
the complete VM3/session execution boundary to one stable root and cover every
top-level and nested run-loop producer. That work is owned by exact delivery
successor `bd-59co.2.7.4`, `CORE-5 establish a stable VM3 synchronous reentry
root`, traced to `CORE-RUNTIME-HELPER-SESSION`, plus its Windows synchronous
event/callback consumers.

## Lifecycle and subscription reconciliation

Object construction resolves metadata and the optional bridge before consuming
an instance identity. Predeclared instances remain visible during recursive
`Class_Initialize`; on failure the failed identity is removed only when re-entry
has not replaced the slot.

Termination cleanup extracts subscription tokens and sink owners under a short
state borrow, releases that borrow, calls host `Unadvise`, then re-snapshots the
owner's keys. The fixed-point loop also owns and unsubscribes registrations
created synchronously during `Unadvise` re-entry before removing the terminating
owner's `WithEvents` bindings. Extracted sink owners remain alive through their
host calls, while an unrelated same-token registration is not accidentally
removed.

## Adversarial coverage

The added tests prove:

- context-free and contextual built-in routing cannot cross the host/context
  ownership boundary, and the complete native catalog classifies only `Rnd` and
  `Randomize` as contextual;
- VM3 can execute `Rnd`, `DoEvents`, then `Rnd` again; the host probe proves
  `DoEvents` exits before VM3 polls callbacks at `StmtBoundary`, and the RNG
  context remains usable on both sides;
- the controlled Windows `StandardHostServices` COM event lifecycle proves the
  real adapter queues, pumps, exposes, releases, and drains its callback payload
  without receiving a VM pointer;
- a missing initializer bridge does not consume an instance identity;
- recursive predeclared initialization exposes the in-flight singleton, failed
  initialization removes that identity, and a re-entrant replacement survives;
- termination drain recursion restores full caller error policy and the RAII
  drain guard;
- host unsubscribe re-entry can install a second subscription for the
  terminating owner and cleanup unsubscribes both tokens before removing the
  owner;
- a fake JIT entry recursively takes the shared As-New path through the stable
  raw root and registration clearing fails closed afterwards;
- both local and referenced-program call helpers propagate callee `Err`, restore
  caller handler mode and frame depth, and remove only the callee mapping from a
  real ParamArray alias chain.

The fake callback/entry tests avoid Cranelift execution and form bounded targets
for Miri when an enabled toolchain is available.

## ABI and observable behavior

The 63 public C helper names, calling conventions, and parameter/result layouts
are unchanged. No C ABI version change is required. The internal explicit-drain
generated-helper signature carries the current run pointer; it is private and
changed coherently with its declaration, emission, and implementation.

The Rust source surface was intentionally hardened in two ways: the procedure
bridge and its state cell became private, and the public generic
`oxvba_lib::invoke` wrapper was removed. The latter is a deliberate Rust source
API break for any untracked external crate; callers must choose
`invoke_context_free` or `invoke_contextual` and therefore state which authority
they need. All repository consumers were migrated or already used the split
APIs. This is not a C ABI or versioned product-contract change.

Observable repairs are:

- valid zero-argument built-ins accept `(null, 0)` and invalid callback state
  fails closed;
- initializer faults propagate, termination faults remain suppressed, and
  caller activation state is restored without discarding the callee `Err` that
  caused a normal failed call;
- identities, singleton replacement, frames, ParamArray aliases, drain guards,
  COM tokens, and sink owners balance under the exercised re-entry paths;
- no public C transport or allocation/AddRef/Release algorithm changed.

## Exact command evidence

Executed in the isolated worker worktree after `37796392`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Pass. |
| `cargo check -p oxvba-lib -p oxvba-vm3 -p oxvba-rt-abi -p oxvba-jit` | Pass. |
| `cargo doc -p oxvba-lib --no-deps` | Pass; the split public dispatcher documentation has no broken intra-doc links. |
| `cargo clippy -p oxvba-lib -p oxvba-rt-abi --all-targets -- -D warnings` | Pass; zero warnings. |
| `cargo clippy -p oxvba-vm3 --all-targets -- -D warnings` | Not green: exactly two inherited warnings, listed below. No pass is claimed. |
| `cargo clippy -p oxvba-vm3 --all-targets -- -D warnings -A clippy::too_many_arguments -A clippy::collapsible_if` | Pass after naming only the two inherited warning classes; no new warning is hidden. |
| `cargo clippy -p oxvba-jit --all-targets -- -D clippy::undocumented_unsafe_blocks -D clippy::missing_safety_doc -D unsafe_op_in_unsafe_fn` | Pass for all selected safety lints; the 21 inherited warnings below remain. |
| `cargo test -p oxvba-lib -p oxvba-vm3 -p oxvba-rt-abi -p oxvba-jit --no-fail-fast` | Pass: 45 library tests, 35 VM3 unit tests, 8 VM3 cross-program tests, 40 runtime ABI tests, 167 JIT tests, and 1 compile-fail doctest; 0 failed. |
| `cargo test -p oxvba-hal windows_native_com_event_subscription_lifecycle_is_tracked -- --nocapture --test-threads=1` | Pass: 1 controlled Windows StandardHost event/queue lifecycle test; 0 failed. |
| `cargo test -p oxvba-differential --test jit_project_objects -- --nocapture` | Pass: 45 tests; 0 failed. |
| `cargo test -p oxvba-differential --test class_lifecycle_vm3 -- --nocapture` | Pass: 6 tests; 0 failed. |
| `cargo test -p oxvba-differential --lib jit_scope_snapshot -- --nocapture` | Pass: 1 test; 0 failed. The earlier broad-target timeout is superseded by this exact library-target run. |
| `cargo miri --version` | Unavailable: stable MSVC has no `miri` component. No Miri run is claimed. |
| `git diff --check` | Pass before the code commit. |

## Inherited warnings and non-claims

Workspace strict Clippy is not green. VM3 has two inherited warnings:

- `clippy::too_many_arguments`: line 134;
- `clippy::collapsible_if`: line 3068.

The exact unchanged JIT warning roster is:

- `clippy::too_many_arguments`: lines 3004, 3021, 3111, 3557, 3951, 4016,
  4884, 15151, 15362, and 19788;
- `clippy::collapsible_if`: lines 7080, 7090, 7100, 7119, 7124, 7134, and
  7147;
- `clippy::needless_borrow`: line 9122;
- `clippy::nonminimal_bool`: lines 13460 and 13489;
- `clippy::needless_lifetimes`: line 14812.

Those baseline warnings belong to the separate strict-Clippy cleanup bead and
were not absorbed here.

The repair is a complete answer to the unsafe generic library-dispatch and
current JIT/runtime-ABI borrow findings in this bounded slice. It does not claim
a stable raw VM3 session root, synchronous same-VM VM3 callback safety, a
versioned product helper catalog, general typed-primary JIT ABI, cross-platform
COM plan, apartment policy, Windows callback proof, or Miri proof. Those remain
owned by the Ideal Core and Windows worksets.

## Independent review verdict

The worker pass searched for safe raw entry points, null/zero slice
construction, public bridge bypass, all repository callers of the removed
generic library API, complete native-ID classification, typed
state/run/context borrows crossing callbacks, every compiled-entry call, VM3's
top-level and nested run-loop caller chain, current StandardHost event-pump
authority, As-New and default-member paths, dynamic host/library invocation,
subscription extraction/reconciliation, error/frame/map cleanup on failure,
symbol/signature drift, missing safety comments, and lint suppression. Findings
from that pass are repaired above, assigned to the stable-root successor, or
explicitly retained as non-claims.

The independent `fresh_review_vm3_lib_split` agent then reviewed code commits
`60b4d88a` and `37796392` plus the final evidence. It confirmed the disjoint
catalog routing, fail-closed VM3 branch, queued/polled/released callback order,
StandardHost's lack of VM authority, honest Rust API-break disclosure, and
exact residual owner `bd-59co.2.7.4`. Its focused rerun passed all 45 library
tests, all 43 VM3 tests, and `git diff --check`. Verdict: **PASS with no
remaining actionable finding**.
