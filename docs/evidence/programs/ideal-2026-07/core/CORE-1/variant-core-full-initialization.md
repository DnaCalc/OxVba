# CORE-1 VariantCore full initialization

Date: 2026-07-11

Bead: `bd-59co.2.2.21`

Effect: delivery

Result: accepted runtime-safety slice; the Core capability profile remains
in-progress

## Contract and counterexample

This slice advances `RUNTIME-VALUE-001`, `SEC-BOUNDARY-001`, and
`CONF-QUALITY-001` while preserving the x64 carrier required by
`RUNTIME-ABI-001`.

Before this change, public safe downstream code could write:

```rust,ignore
let data = VariantData { i16_val: 7 };
let core = VariantCore {
    vtype: VarType::Integer,
    reserved1: 0,
    reserved2: 0,
    reserved3: 0,
    data,
};
```

Initializing the union's two-byte field left six bytes uninitialized. Safe
`VariantCore::data_bytes`, `Debug`, `PartialEq`, and `to_wire_bytes` then read
all eight payload bytes. The workspace happened to construct the union through
its eight-byte field, but the public safe API did not enforce that invariant.

## Delivered design and migration

- `VariantData` is now a `repr(C, align(8))` eight-byte struct with private
  storage, not a union. Its safe construction is therefore fully initialized by
  type construction rather than an audited comment around an unsafe union read.
- A zero-sized `PhantomData<*mut c_void>` restores the former union's
  `!Send + !Sync` auto-trait boundary. It occupies no carrier bytes, so
  `VariantData`, `VariantCore`, and owning `Variant` retain their previous
  single-thread ownership contract without changing size, alignment, offsets,
  raw memory, or wire bytes.
- `VariantData::from_bytes` preserves the exact raw-byte route.
  `from_i16`, `from_i32`, `from_i64`, and `from_f64` are explicit migrations
  for the former public scalar union fields and zero every unused trailing
  byte. `from_exposed_pointer` creates all eight address bytes under the live
  Exposed Provenance contract established by `bd-59co.2.2.20`.
- `VariantCore` retains `repr(C)` but makes all fields private. Public
  `from_bytes` and `from_parts` initialize the complete header and payload;
  public `vtype`, `reserved1`, `reserved2`, `reserved3`, and `data_bytes`
  accessors replace direct reads. `VariantData` is re-exported from the crate
  root beside `VariantCore` to make migration discoverable.
- Two external-context compile-fail doctests permanently reject both the old
  short union initializer and direct `VariantCore` field construction. The
  integration test is compiled as a separate downstream crate and exercises
  every new public constructor/accessor needed for raw carrier use.
- Six additional external compile-fail probes independently reject `Send` and
  `Sync` for `VariantData`, `VariantCore`, and owning `Variant`. Cross-thread
  carrier/object ownership remains explicit delivery work under
  `bd-59co.2.7.2`; this safety slice does not make scalar-looking Variants
  implicitly transferable.
- Internal Empty, Null, signed/unsigned scalar, floating-point, Currency, Date,
  Boolean, Error, ProcRef, Decimal, String/BSTR, Object/IUnknown, SAFEARRAY, and
  record constructors were audited. They all reach `VariantCore::from_bytes`
  or `from_parts` with an initialized `[u8; 8]`. Wire decoding copies exactly
  eight initialized payload bytes before construction. No workspace crate
  constructs or reads `VariantCore` fields directly outside `variant.rs`.
- The unallocated-array constructor now uses `from_parts` rather than mutating
  a header field after construction. Decimal construction uses the same sealed
  path for its non-zero reserved words.
- `data_bytes` is now a safe direct copy from the byte struct and contains no
  unsafe union read. `Debug`, equality, and wire encoding consume only those
  initialized bytes.
- Public rustdoc now states the complete raw-carrier contract on every
  `VariantData` constructor and both `VariantCore` constructors: simulated x64
  little-endian byte order; zero-filled, not sign-extended, unused short-scalar
  bytes; initialized-but-unvalidated and non-owning raw values; no safe
  promotion of pointer tags or noncanonical parts into an owning/dereferenceable
  Variant; and the separation between validated untrusted wire decoding and
  unsafe trusted live-pointer recovery.

The Rust source API necessarily removes the unsound public union-field and
direct-struct literals. This is an explicit source migration, not a silent
break. The binary contract is unchanged: external and Miri tests prove
`VariantData` remains size 8/alignment 8 and `VariantCore` remains size
16/alignment 8 with the same header/data byte order. Unsafe FFI code may still
address the `repr(C)` layout directly, but it owns the normal unsafe precondition
that every byte it presents as a Rust value is initialized.

## Observable evidence

| surface | evidence |
|---|---|
| Result | The external integration target constructs raw Decimal carrier bytes through `from_parts`, reads every accessor, compares Debug/Eq/wire results, and reads the initialized 16-byte `repr(C)` memory image. All scalar Variant families plus BSTR, Object, SAFEARRAY, and VBA Record cores pass `data_bytes`, Debug, Eq, and wire round trips with exact values. Short scalar tails are exactly zero. Compile-fail probes prove Data/Core/Variant are each independently neither Send nor Sync. |
| Full Err | This is carrier construction, not VBA execution, so it does not create or mutate VBA `Err`. Safe `from_wire_bytes` retains its deterministic unsupported-VARENUM and malformed-reserved-word `Result::Err` behavior. Pointer-bearing raw cores remain bytes only; converting trusted live pointer bytes into an owning `Variant` remains the explicitly unsafe contract from `bd-59co.2.2.20`. |
| Side effects | Constructing or reading `VariantData`/`VariantCore` performs no allocation, retain/release, host call, or runtime-state mutation. Pointer-owning Variant fixtures allocate only through their existing semantic constructors. Compile-fail probes have no runtime side effects. |
| Lifecycle/event order | The external test keeps each pointer carrier live across all core reads, then drops BSTR, Object, SAFEARRAY, and record owners in ordinary Rust scope order. No runtime event dispatch is involved. Construction now has a single initialized state; there is no intermediate safe partial-payload state to observe. The restored auto-trait fence prevents safe cross-thread movement from bypassing this ownership order. |
| Transport | The 16-byte native and wire images remain byte-for-byte identical on the active x64 little-endian profiles. `from_parts` is the raw initialized carrier constructor; `from_wire_bytes` remains the validated byte decoder. Process-local pointer bytes retain the transport restrictions recorded by `bd-59co.2.2.20`. |
| Balance | The isolated integration process begins and ends with zero deltas for BSTRs, compat object boxes, SAFEARRAYs, and VBA record buffers. Scalar/raw-core construction has no handle effect. The complete runtime suite remains green. |

## Checks

- `cargo test -p oxvba-runtime variant_core_full_initialization -- --nocapture --test-threads=1` — 1 passed in an external integration target.
- `cargo +nightly miri test -p oxvba-runtime variant_core_full_initialization -- --nocapture` — 1 passed with no uninitialized-byte read or undefined behavior. Four visible warnings are the already accepted Exposed Provenance recovery sites from `bd-59co.2.2.20`, not initialization warnings.
- `cargo test -p oxvba-runtime variant::tests -- --test-threads=1` — 15 passed.
- `cargo test -p oxvba-runtime -- --test-threads=1` — 170 unit tests and 2 isolated integration tests passed; 8 compile-fail doctests passed.
- `cargo test -p oxvba-runtime --doc` — two construction/privacy and six independent auto-trait regressions passed.
- `cargo clippy -p oxvba-runtime --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.

## Residual disposition

- No accepted residual remains in safe `VariantData`/`VariantCore` full-byte
  initialization. Outside-workspace Rust source that used the former public
  fields must use the documented constructors/accessors; the binary ABI does
  not change.
- Raw `from_parts` intentionally guarantees initialization and layout, not
  semantic validity of every tag/reserved-word combination. Validated untrusted
  byte admission remains `from_wire_bytes`; owning pointer recovery remains
  unsafe. This separation prevents a low-level ABI constructor from silently
  becoming an untrusted decoder.
- Strict pointer provenance remains the completed scope of
  `bd-59co.2.2.20`. Transactional owning-record replacement and unwind-safe
  borrowed carrier projection remain `bd-59co.2.2.16` and
  `bd-59co.2.2.17`; this slice neither modifies nor claims them.
- Intentional cross-thread object/carrier ownership remains
  `bd-59co.2.7.2`. Restoring the historical `!Send + !Sync` boundary prevents
  this raw initialization refactor from silently pre-empting that design.
