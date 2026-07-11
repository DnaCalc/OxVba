# SafeArray unsafe ownership audit

Date: 2026-07-11

Bead: `bd-2cjy`

Baseline: `30525eed91bcffa56bc21ab582dddbf81bdb1f75`

Status: targeted repair and audit complete; one public thread-ownership question is escalated as a residual.

## Scope and contract trace

This delivery covers `crates/oxvba-runtime/src/safe_array.rs` and the six strict-
Clippy findings characterized by the CORE-1 rollout. It traces to
`CONF-QUALITY-001`, `RUNTIME-VALUE-001`, and `SEC-BOUNDARY-001`, and to
`CORE-READINESS/CORE-BASELINE-UNSAFE-CLIPPY`.

The repair adds no lint suppression and changes no layout, ABI, allocation,
indexing, error, or ownership behavior. It documents the invariants at the four
unsafe blocks and the two public unsafe functions that Clippy rejected. One
pre-existing layout comment was also corrected from a fixed "8-byte prefix" to
the actual size-derived owner-prefix offset.

## Six-finding repair record

| Site | Finding | Invariant recorded |
|---|---|---|
| `raw_bounds_slice` | undocumented `from_raw_parts` | the live descriptor allocation contains exactly `c_dims` contiguous initialized bounds beginning at `rgsabound` |
| `SafeArray::i32_element` | undocumented typed read | I4/Int selects the aligned `i32` layout; the checked index addresses a live slot in a non-null payload |
| `SafeArray::set_i32_element` | undocumented typed write | the same layout/index proof applies and exclusive `&mut self` makes the slot uniquely writable |
| `SafeArray::raw_safearray_i32_element` | missing `# Safety` | the raw pointer is a live OxVba-owned descriptor and the caller holds shared access for the complete borrow |
| `SafeArray::set_raw_safearray_i32_element` | missing `# Safety` | the raw pointer is a live OxVba-owned descriptor and the caller holds exclusive access for the complete mutation |
| `SafeArray::drop` owner deallocation | undocumented deallocation | the recovered owner pointer and recomputed dimension-dependent layout are the exact allocation pair, and no access follows deallocation |

The two raw helpers explicitly leave descriptor ownership with the caller. Their
temporary `SafeArray` wrappers are still forgotten after the operation, so the
documentation matches the implemented non-owning projection.

## Full-file fresh-eyes ownership review

The audit covered the complete SafeArray implementation, including:

- descriptor layout, dimension-dependent owner allocation, provenance magic and
  version validation, bound slicing, flattened-length arithmetic, and payload
  offset calculation;
- zeroed payload allocation and partial-initialization cleanup for all intrinsic
  scalar kinds, BSTR, object, Variant, and record elements;
- replacement ordering for owned BSTR, object, Variant, and record slots, so a
  replacement is prepared before the old payload is released where failure is
  possible;
- object AddRef/Release and BSTR clone/free ownership when encoding, decoding,
  cloning, replacing, and dropping elements;
- record-layout `Arc` provenance, raw strong-count reconstruction, contiguous
  record clone/drop, and error cleanup;
- raw descriptor adoption, borrowing, cloning, mutation, and ownership-transfer
  entry points;
- `Clone` and `Drop` ordering for payload elements, payload storage, record-layout
  metadata, descriptor storage, and the SafeArray live counter.

No additional defect was found in the allocation, initialization, clone,
replacement, or drop paths themselves. The six repairs are therefore invariant
documentation rather than behavior changes; the separate public thread-trait
question found at the file boundary is recorded below.

## Acceptance evidence

Run in the isolated `codex/bd-2cjy-safe-array` worktree on Windows x64:

| Command | Result |
|---|---|
| `cargo fmt -p oxvba-runtime -- --check` | pass |
| `cargo clippy -p oxvba-runtime --all-targets -- -D warnings` | pass; zero warnings |
| `cargo test -p oxvba-runtime safe_array` | pass; 28 passed, 0 failed, 0 ignored, 136 filtered out |

Observable axes:

- Result: the strict runtime Clippy regression is absent and all targeted
  SafeArray tests retain their prior results.
- Full Err: no error construction, number, description, source, help fields, or
  Erl behavior changed; the touched APIs retain their existing `Result` paths.
- Side effects: descriptor/payload allocation, element mutation, and cleanup
  code and execution order are unchanged. Only the source placement of the
  existing live-counter decrement relative to the deallocation comment changed,
  so the safety argument now immediately precedes the unsafe block.
- Lifecycle/order: construction, partial-failure cleanup, clone, replacement,
  element destruction, payload deallocation, metadata release, and descriptor
  deallocation were reviewed; the targeted suite exercises scalar, BSTR,
  object, Variant, record, clone, mutation, and raw-descriptor paths.
- Transport: the two raw I4/Int borrow contracts are now explicit; pointer shape,
  representation, provenance validation, ABI, and ownership transfer are
  unchanged.
- Balance: the allocation/freed counter calls remain paired at descriptor owner
  creation and destruction. This bead does not claim an isolated global-counter
  proof; that workspace lifecycle/balance gate remains with the separate CORE-1
  baseline successors.

## Escalated residual

`SafeArray` currently has unconditional unsafe `Send` and `Sync` implementations,
including when it stores VT_DISPATCH/VT_UNKNOWN `ObjectRef` elements. The existing
comment requires object-bearing arrays to be dropped on the managing VM thread,
but the public traits do not enforce that condition. Compat objects contain
`RefCell`/`Cell` state, final release uses a thread-local termination queue, and
foreign COM objects may be apartment-affine.

Changing those traits or introducing an owner-thread carrier would alter a public
runtime contract, so this bead deliberately does not do so. The issue is
escalated for a high-risk CORE-5/host-session decision and a Variant/interop
transit audit. It must not be treated as resolved by the six Clippy repairs.

The already integrated `vba_record` certification and workspace-wide strict
verification also remain the separate successors named by the rollout graph.
