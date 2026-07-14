# Runtime ABI unsafe-boundary and re-entry repair

Date: 2026-07-14

Bead: `bd-59co.2.2.26`

Baseline: `b5604905554d29d14277fbf17d4720184cdf12ed`

Repair commits: `a2c012bb`, `648936d7`, and `b94abfda`

Branch: `codex/bd-59co-2-2-26-rt-abi-safety`

Status: the bounded raw runtime ABI, synchronous procedure bridge, stable JIT
run-root, library-context split, and termination-cleanup repair is implemented
and passes its focused gates. Independent controller review and integration
remain required. This evidence does not claim full `RUNTIME-ABI-001`, Windows
COM behavior, product helper-catalog completion, or workspace strict-Clippy
closure.

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
- the existing `invoke` API remains as a compatibility wrapper over those two
  paths, so this is additive rather than a versioned public break;
- `rt_lib_invoke_with_policy` copies the host under a short state borrow, drops
  that borrow before context-free host dispatch, and borrows RNG context only
  for the non-host contextual branch.

This removes the previous unsound possibility that a host call such as
`DoEvents` could re-enter VBA while an `ExecState`/`LibContext` borrow remained
live.

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
  ownership boundary;
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

The Rust source surface was intentionally hardened by making the bridge and its
state cell private. An untracked external Rust caller that constructed those
private details would need to use the install/clear ABI, but no repository
consumer did so.

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

Executed in the isolated worker worktree after `b94abfda`:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Pass. |
| `cargo check -p oxvba-lib -p oxvba-rt-abi -p oxvba-jit` | Pass. |
| `cargo clippy -p oxvba-lib -p oxvba-rt-abi --all-targets -- -D warnings` | Pass; zero warnings. |
| `cargo clippy -p oxvba-jit --all-targets -- -D clippy::undocumented_unsafe_blocks -D clippy::missing_safety_doc -D unsafe_op_in_unsafe_fn` | Pass for all selected safety lints; the 21 inherited warnings below remain. |
| `cargo test -p oxvba-lib -p oxvba-rt-abi` | Pass: 44 library tests, 40 runtime ABI tests, and 1 compile-fail doctest. |
| `cargo test -p oxvba-jit` | Pass: 167 tests; 0 failed. |
| `cargo test -p oxvba-differential --test jit_project_objects -- --nocapture` | Pass: 45 tests; 0 failed. |
| `cargo test -p oxvba-differential --test class_lifecycle_vm3 -- --nocapture` | Pass: 6 tests; 0 failed. |
| `cargo test -p oxvba-differential jit_scope_snapshot -- --nocapture` | Inconclusive: command-level timeout during broad target enumeration/build, with no test result emitted. No pass is claimed. |
| `cargo miri --version` | Unavailable: stable MSVC has no `miri` component. No Miri run is claimed. |
| `git diff --check` | Pass before the code commit. |

## Inherited warnings and non-claims

Workspace strict Clippy is not green. The exact unchanged JIT warning roster is:

- `clippy::too_many_arguments`: lines 3004, 3021, 3111, 3557, 3951, 4016,
  4884, 15151, 15362, and 19788;
- `clippy::collapsible_if`: lines 7080, 7090, 7100, 7119, 7124, 7134, and
  7147;
- `clippy::needless_borrow`: line 9122;
- `clippy::nonminimal_bool`: lines 13460 and 13489;
- `clippy::needless_lifetimes`: line 14812.

Those baseline warnings belong to the separate strict-Clippy cleanup bead and
were not absorbed here.

The repair is a complete answer to the fresh-review borrow/re-entry findings in
this bounded slice. It does not claim a versioned product helper catalog,
general typed-primary JIT ABI, cross-platform COM plan, apartment policy,
Windows callback proof, or Miri proof. Those remain owned by the Ideal Core and
Windows worksets.

## Worker review verdict

The final worker pass searched for safe raw entry points, null/zero slice
construction, public bridge bypass, typed state/run/context borrows crossing
callbacks, every compiled-entry call, As-New and default-member paths, dynamic
host/library invocation, subscription extraction/reconciliation, error/frame/map
cleanup on failure, symbol/signature drift, missing safety comments, and lint
suppression. Findings from that pass are repaired above or explicitly retained
as non-claims. A new independent non-author review is required before bead
acceptance.
