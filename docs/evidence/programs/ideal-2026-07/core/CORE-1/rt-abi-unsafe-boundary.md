# Runtime ABI unsafe-boundary repair

Date: 2026-07-14

Bead: `bd-59co.2.2.26`

Baseline: `b5604905554d29d14277fbf17d4720184cdf12ed`

Code commit: `a2c012bb564d5c12d4c6246e4f37eb176bfa8fa5`

Branch: `codex/bd-59co-2-2-26-rt-abi-safety`

Status: targeted repair complete. This is evidence for the runtime-ABI safety
boundary only, not workspace-wide closure or a new VBA parity claim.

## Scope and finding disposition

The baseline strict command reported 44 `oxvba-rt-abi` Clippy errors. They
covered a derivable `EventFabric::default`, an eight-input
`runtime_member_descriptor`, public raw-pointer helpers that were callable from
safe Rust, and undocumented pointer conversions/dereferences. No lint
suppression was added.

The repair applies one consistent boundary model:

- all 63 exported `#[unsafe(no_mangle)]` runtime helpers are now
  `pub unsafe extern "C" fn`, each with a `# Safety` contract covering its
  actual state, value, output, callback-context, pointer/length, and aliasing
  obligations;
- `RawExecState` records the shared provenance, alignment, initialization,
  liveness, exclusivity, same-thread, allocation-extent, and ownership rules;
- private raw cores (`state_from_raw`, `seat_fault`, `write_out<T>`,
  `read_in<T>`, and `release_variant_slot`) are unsafe and document their exact
  contracts. In particular, `state_from_raw` performs a null check and a
  conversion; it does not claim to validate arbitrary non-null pointers;
- output contracts require initialized, uniquely writable storage because the
  existing generic assignment path replaces the prior typed value;
- Rust and JIT callers cross the boundary only in documented unsafe blocks.
  The analogous JIT raw-state conversion and 32 private raw-state forwarding
  helpers were also made unsafe so safe in-crate Rust cannot smuggle an
  arbitrary handle to a dereference;
- a compile-fail doctest proves safe Rust cannot call `rt_err_clear`, while the
  existing behavior suites exercise status, Err, release, arithmetic,
  activation, routing, library, and JIT paths through the explicit boundary.

`EventFabric` now derives `Default`. The removed manual implementation used
`HashMap::new`, `0`, `false`, `Vec::new`, and `None` for the corresponding
fields, exactly matching each field type's derived default.

`RuntimeMemberDescriptorInput` groups the method/display/dispatch/vtable/
default/enumerator inputs. Both construction paths retain their prior field
mapping and precedence rules; this is a Rust call-shape change only.

## ABI and behavior preservation

The baseline and repaired source each contain 63 exported `rt_*` functions and
63 `#[unsafe(no_mangle)]` attributes. A name-set comparison found no missing or
added exported helper. `unsafe` changes Rust's caller obligation; it does not
change the C calling convention, symbol name, parameter layout, return layout,
or generated-code address registration.

No helper body was given a new fallback, status value, or panic policy.
`with_status`/JIT `status_guard` catch-unwind boundaries remain in place, and
the existing `ST_OK`, `ST_FAULT`, `ST_HALT`, full `Err`, activation restore,
callback, release, and termination-drain ordering remains unchanged. The
runtime and JIT suites below are the executable regression evidence.

Observable axes:

- Result: all focused runtime and JIT tests retain their expected status and
  return values; strict runtime Clippy is warning-free.
- Full Err: arithmetic, type mismatch, stack, explicit raise, routing, resume,
  and activation tests retain number, description, source, and policy behavior.
- Side effects: output writes, argument handling, project initialization, and
  JIT slot updates retain their prior order; unsafe blocks only expose the
  pre-existing caller invariants.
- Lifecycle/order: type-specific release, callback activation, frame cleanup,
  restoration, and termination draining remain covered by the passing suites.
- Transport: C symbols and ABI signatures are unchanged; Rust raw-pointer use
  is now statically unsafe at every exported and private dereference boundary.
- Balance: this bead changes no allocation or AddRef/Release algorithm. Existing
  release/lifecycle tests pass; no workspace-wide counter certification is
  claimed here.

## Commands and exact results

Executed in the isolated worktree rooted at baseline
`b5604905554d29d14277fbf17d4720184cdf12ed`:

| Command | Result |
|---|---|
| `cargo fmt --all` | Pass. |
| `cargo clippy -p oxvba-rt-abi --all-targets -- -D warnings` | Pass; exit 0, zero warnings. The baseline had 44 errors. |
| `cargo test -p oxvba-rt-abi` | Pass: 34 unit tests and 1 compile-fail doctest; 0 failed. |
| `cargo test -p oxvba-jit` | Pass: 164 unit tests; 0 failed; doc-tests contain 0 tests. |
| `cargo check -p oxvba-jit --message-format=short` | Pass. |
| `cargo clippy -p oxvba-jit --all-targets --message-format=short -- -D clippy::undocumented-unsafe-blocks` | Pass for the safety lint. It reports 21 inherited non-safety warnings outside this bead's strict runtime gate. |
| Baseline/current exported-name and `no_mangle` comparison | `BASE_EXPORTS=63 CURRENT_EXPORTS=63`, no missing/added names, `NO_MANGLE_BASE=63 NO_MANGLE_CURRENT=63`. |
| Added-line suppression scan for `allow`, `expect`, `clippy::`, and `undocumented_unsafe_blocks` | No added suppression or override. |
| `git diff --check` | Pass. |

## Fresh-eyes verdict

The final audit specifically looked for safe private functions hiding raw
dereferences, false pointer-validation claims, incomplete null/alignment/
lifetime/aliasing/length/output-initialization contracts, undocumented unsafe
blocks, symbol drift, callback-context lifetime gaps, panic-boundary movement,
and release/order changes. It found and repaired the hidden private runtime and
JIT forwarding cores described above. The final signature scans leave no safe
private runtime function accepting raw pointers other than
`exec_state_as_raw`, which only converts a live typed mutable reference into an
opaque handle.

No public contract/version conflict was found. The governing system contract's
`RUNTIME-ABI-001` requirement for unsafe raw-pointer entry points behind typed
wrappers is satisfied by this repair.
