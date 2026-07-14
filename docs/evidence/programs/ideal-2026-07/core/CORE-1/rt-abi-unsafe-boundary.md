# Runtime ABI unsafe-boundary repair

Date: 2026-07-14

Bead: `bd-59co.2.2.26`

Baseline: `b5604905554d29d14277fbf17d4720184cdf12ed`

Initial code commit: `a2c012bb564d5c12d4c6246e4f37eb176bfa8fa5`

Re-entry follow-up commit: `648936d7`

Branch: `codex/bd-59co-2-2-26-rt-abi-safety`

Status: the bounded raw-pointer and procedure-callback repair is implemented and
passes its focused gates. This record does not claim workspace strict-Clippy
closure, complete host/HAL re-entry policy, or complete `RUNTIME-ABI-001`.
Independent controller review and integration gates remain required.

## Implemented repair

The initial commit established the explicit unsafe boundary:

- all 63 exported `#[unsafe(no_mangle)]` runtime helpers are
  `pub unsafe extern "C" fn` with `# Safety` contracts;
- `RawExecState` and the private raw conversion/read/write/release cores state
  provenance, alignment, initialization, liveness, aliasing, same-thread,
  extent and ownership obligations;
- the corresponding JIT raw-state forwarders and function-pointer types are
  unsafe, with no lint suppression or exported C symbol-name change;
- `EventFabric` derives the field-equivalent `Default`, and the former
  eight-argument runtime-member construction is grouped in an owning input.

Review of that implementation found additional soundness gaps. Commit
`648936d7` repairs them:

- `rt_lib_invoke[_with_policy]` now maps `(null, 0)` to `&[]` rather than
  calling `slice::from_raw_parts(null, 0)`;
- `ProcInvokeBridge`, `ExecState::proc_invoker` and the typed drain core are
  private. The only installation surface is the unsafe install/clear ABI, and
  `ProcInvokeFn` documents context, `Me`, same-thread, no-unwind, re-entry,
  status and in-flight-lifetime obligations;
- termination drain, `New`, and predeclared-instance construction use short
  execution-state borrows. No `&mut ExecState` survives a procedure callback;
  drain reset is RAII-managed and saved `Err` is restored after suppressed
  termination callbacks;
- failed predeclared initialization removes the failed singleton only if a
  re-entrant action has not replaced it. Missing-invoker failure no longer
  publishes a singleton or consumes an instance identity;
- the JIT installs the bridge through the ABI and owns an RAII registration
  guard that clears the opaque context before its stack storage expires;
- the callback context is copied as raw handles rather than mutably borrowed
  across recursion. The current `JitRun` reborrow is refreshed before all five
  generated-entry call sites, shared and direct As-New/New/predeclared paths,
  statement/final/explicit termination drains, and dynamic host invocation;
- explicit termination drain now carries the current internal run handle. This
  is a private generated-helper signature change; its declaration, emission and
  registered implementation changed together;
- integer-carried UTF-8 operand/name decoders are unsafe with explicit storage
  contracts, and zero-length descriptors use `&[]` without dereferencing a
  null pointer.

The new tests cover null/zero built-in invocation, predeclared initializer
re-entry and failure cleanup, termination-drain recursion/Err restoration/RAII
reset, a nested fake JIT initializer through the shared As-New path, frame and
alias cleanup, bridge clearing before context expiry, and the changed explicit
drain helper signature.

## ABI, API and observable behavior

The 63 public C helper names, calling conventions and parameter/result layouts
from the initial repair remain unchanged. No C ABI version change is required.
The internal `rt_jit_drain_terminations` generated-helper signature is not an
exported product ABI and was changed coherently with its only producer.

This follow-up intentionally hardens the Rust source API: external Rust code can
no longer construct `ProcInvokeBridge` or mutate `ExecState::proc_invoker`
directly. Repository search found no consumer other than the JIT, which now uses
the install/clear ABI. This is a source-compatibility break for any untracked
out-of-repository caller, but not a C ABI break.

Observable changes are repairs, not preservation claims:

- Result: valid zero-argument built-ins accept the documented null pointer;
  invalid/missing callback state fails closed.
- Full Err: termination callback faults remain suppressed and the caller's
  complete saved error engine is restored; initializer faults retain their
  returned status.
- Side effects: predeclared instances are visible during recursive
  `Class_Initialize`, but a failed instance is removed unless re-entry replaced
  it. Missing bridge failure does not leave a singleton behind.
- Lifecycle/order: registration clears before callback context drop; nested
  initializer frames and parameter-array aliases balance; termination drain
  returns its guard to non-draining state.
- Transport: public C transport is unchanged. Private integer-pointer decoders
  now expose their actual unsafe preconditions.
- Balance: no allocation/AddRef/Release algorithm changed. Focused lifecycle
  tests pass, but no workspace-wide live-counter claim is made here.

## Exact command evidence

Executed in the isolated worker worktree:

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Pass. |
| `cargo check -p oxvba-rt-abi -p oxvba-jit` | Pass. |
| `cargo clippy -p oxvba-rt-abi --all-targets -- -D warnings` | Pass; zero warnings. |
| `cargo clippy -p oxvba-jit --all-targets -- -D clippy::undocumented_unsafe_blocks -D clippy::missing_safety_doc -D unsafe_op_in_unsafe_fn` | Pass for the selected safety lints. It also reports the 21 baseline JIT warnings listed below. |
| `cargo test -p oxvba-rt-abi` | Pass: 37 unit tests and 1 compile-fail doctest; 0 failed. |
| `cargo test -p oxvba-jit` | Pass: 166 unit tests; 0 failed; doc-tests contain 0 tests. |
| `cargo miri --version` | Not available: stable MSVC reports that the `miri` component/cargo-miri is unavailable. No Miri run is claimed. |
| `git diff --check` | Pass. |

The two runtime fake-callback tests deliberately contain no Cranelift execution
and are suitable extracted Miri targets once an enabled nightly Miri toolchain
is available.

## Required successors and non-claims

Workspace strict Clippy is not green. The exact inherited `oxvba-jit` roster is:

- `clippy::too_many_arguments`: lines 3001, 3018, 3108, 3554, 3948, 4013,
  4881, 15105, 15305 and 19668;
- `clippy::collapsible_if`: lines 7077, 7087, 7097, 7116, 7121, 7131 and 7144;
- `clippy::needless_borrow`: line 9119;
- `clippy::nonminimal_bool`: lines 13434 and 13463;
- `clippy::needless_lifetimes`: line 14794.

These findings are outside the changed soundness lines and need a bounded
delivery successor before the baseline lane can close.

The explicit procedure bridge and JIT dynamic COM call are now re-entry-safe,
but a broader architectural residual remains: `rt_lib_invoke_with_policy`
threads `&mut ExecState::lib` through `oxvba_lib::invoke`, and HAL facets such as
event pumping/COM may eventually permit synchronous project re-entry. The
library-context ownership/re-entry policy must be specified and implemented
before general host re-entry is claimed. Related COM unsubscribe/event paths
must be audited under the Windows interop plan rather than inferred safe from
the current Linux/null-host tests.

This bead therefore realizes the unsafe raw-entry and current ProcInvoke/JIT
re-entry slice of `RUNTIME-ABI-001`; it does not satisfy the clause's future
versioned helper catalog, general typed-wrapper, host/apartment or Windows
interop requirements.

## Worker review verdict

The worker's second review searched for null/zero slice construction, safe
integer-pointer decoders, public callback-bridge bypasses, mutable state/run/
context borrows spanning callbacks, every generated-entry call, As-New and
termination-drain paths, dynamic host invocation, registration drop order,
symbol/signature drift, missing safety comments and lint suppression. The
issues found in that review are repaired above or recorded as explicit
successors. An independent non-author review has been requested and is not
represented as completed evidence here.
