# CORE-1 Variant carrier strict provenance

Date: 2026-07-11

Bead: `bd-59co.2.2.20`

Effect: delivery

Result: accepted runtime-safety slice; the Core capability profile remains
in-progress

## Contract and scope

This slice advances `RUNTIME-VALUE-001`, `RUNTIME-ABI-001`,
`SEC-BOUNDARY-001`, and `CONF-QUALITY-001`.

`VariantCore` remains an exact 16-byte, 8-byte-aligned x64 carrier. Its payload
continues to contain eight little-endian address bytes for the process-local
pointer-backed families. This slice changes neither the public byte ABI nor
ownership semantics.

The audited byte-pack/recovery paths are:

| carrier owner | stored pointer | ownership that keeps it live |
|---|---|---|
| `Variant(String)` | BSTR payload | the Variant owns the BSTR until drop |
| `Variant(Object)` | `RawRuntimeIUnknown` | the Variant owns one retained reference until drop |
| `Variant(ArrayVariant)` | OxVba SAFEARRAY descriptor | the Variant owns the descriptor until drop |
| `Variant(Record)` | `Box<RecordPayload>` for both `Vba` and `Com` | the Variant owns the box until drop |
| typed SAFEARRAY `Dispatch`/`Unknown` element | `RawRuntimeIUnknown` in an eight-byte element slot | the SAFEARRAY owns one retained reference per live slot |

Typed SAFEARRAY BSTR slots store and load a pointer-typed cell directly rather
than converting it through an integer. `ComRecord` retains typed raw pointers
inside its callback-owned record handle. `ProcRef` is a numeric procedure token,
not a Rust pointer. The pointer-helper registry, callback thunk addresses, and
descriptor-address cache keys are explicit VBA/native address or identity
boundaries and do not reconstruct a dereferenceable pointer from `VariantCore`
bytes; they are not silently claimed by this carrier slice.

## Delivered implementation

- Packing now calls `expose_provenance()` and checked `usize -> u64`
  conversion before writing the unchanged little-endian eight-byte carrier.
- Recovery now performs checked `u64 -> usize` conversion and calls
  `with_exposed_provenance` or `with_exposed_provenance_mut` for the exact
  pointee type. Null remains address zero.
- The exposure/recovery contract is sound only while the originating
  allocation or retained object reference remains live. Internal Variant and
  SAFEARRAY owners guarantee that interval. The unsafe
  `Variant::from_trusted_wire_bytes` contract already requires the source
  pointer and allocation to remain live for the complete call. Safe wire
  decoding continues to reject every pointer-bearing type before recovery.
- Keeping the source owner live prevents allocation-address reuse during
  recovery. The recovered pointer is used only with the pointee type and
  ownership mode that originally exposed it. Clone paths take a new deep copy
  or retained reference; drop paths reacquire the original owned carrier once.
- Miri exposed an adjacent pre-existing defect in compat-object pointer
  creation. `ObjectRef` had projected `&mut CompatObjectBase::unknown` and later
  cast that field-bounded pointer back to the complete box to update the
  adjacent refcount. Compat and parked-object construction now cast the
  `Box::into_raw` allocation pointer directly to the `repr(C)` first-field
  `RawRuntimeIUnknown` type, preserving provenance for the complete allocation.
  The foreign-object test fixture uses the same valid construction.
- Windows Miri cannot execute `SysAllocString*`, `SysStringByteLen`, or
  `SysFreeString`. Under the declared `cfg(miri)` only, BSTR uses the existing
  Windows-shaped fallback allocator (four-byte length prefix, byte payload,
  UTF-16 NUL). Production Windows continues to use the OLE functions. This
  makes the carrier and ownership path executable under Miri without replacing
  the production ABI path.

Rust's Exposed Provenance API is deliberately used here rather than a plain
integer cast. Miri reports an informational warning whenever
`with_exposed_provenance` is used because the interpreter cannot prove all
wildcard-provenance choices. The focused run leaves those warnings enabled; it
does not use `-Zmiri-permissive-provenance` to hide them. The run nevertheless
checks the chosen live allocations and caught the invalid field-bounded object
provenance described above. A `-Zmiri-strict-provenance` run cannot admit any
address-only byte carrier by definition; satisfying that stronger experimental
mode would require a provenance-bearing in-memory sidecar or representation,
not the required x64 byte-only `VariantCore` payload. No such ABI change is
needed for the accepted explicit-exposure contract.

## Observable evidence

| surface | evidence |
|---|---|
| Result | The filterable integration test asserts `VariantCore` size 16/alignment 8 and exact address bytes. It constructs, reads, clones, trusted-wire clones, mutates, and drops BSTR, Object/IUnknown, SAFEARRAY, VBA record, and COM record carriers. Exact observations include the odd BSTR bytes `41 00 42`, object identities 73/101/202, independent VBA record values 41/99, and independent COM record values 51/88. |
| Full Err | Carrier recovery is an unsafe in-process substrate operation and does not mutate VBA `Err`. Safe `from_wire_bytes` rejects String, Object, ArrayVariant, and Record bytes with `require trusted in-process provenance`, including arbitrary non-null address `1`; it never attempts recovery or dereference. Trusted null carriers return null/Nothing/unallocated values. Stale or arbitrary non-null bytes remain explicitly outside the unsafe trusted-decoder contract. |
| Side effects | Clone paths allocate or retain independent carrier ownership; source mutations do not alter deep clones. SAFEARRAY object replacement retains the new object before releasing the old slot. The provenance helpers themselves only expose/recover an address and do not allocate, retain, release, or mutate runtime state. No VBA-visible side-effect journal is involved. |
| Lifecycle/event order | Every source remains live across trusted recovery. BSTR and record clones are materialized before source mutation/drop; Object and SAFEARRAY clones take their retained references before originals release. COM record clone/destroy callbacks use an independent live-data counter. No runtime event dispatch occurs. |
| Transport | This is process-local carrier transport only. The exact eight payload bytes remain ABI-compatible, but pointer-bearing bytes are not a persistent or cross-process serialization format. Safe transport rejects them; only the unsafe live-source decoder admits them. Normal Windows tests exercise real OLE BSTR allocation, while Miri exercises the same carrier logic over the layout-equivalent test allocator. |
| Balance | The isolated test process starts and ends with zero deltas for BSTRs, compat object boxes, SAFEARRAYs, and VBA record buffers. Its separate COM record-data counter also returns to zero after ordinary and trusted-wire clones drop. The full runtime suite remains green. |

## Checks

- `cargo test -p oxvba-runtime variant_strict_provenance -- --nocapture --test-threads=1` — 1 passed.
- `cargo test -p oxvba-runtime variant::tests -- --test-threads=1` — 15 passed.
- `cargo test -p oxvba-runtime -- --test-threads=1` — 170 unit tests and 1 integration test passed; doc tests passed.
- `cargo clippy -p oxvba-runtime --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo +nightly miri test -p oxvba-runtime variant_strict_provenance -- --nocapture` — 1 passed. Five unsuppressed informational exposed-provenance warnings identify the four Variant recovery helpers and the typed SAFEARRAY object-element recovery helper; no undefined behavior or assertion failure remained after the full-box `ObjectRef` repair.

## Residual disposition

- No accepted residual remains in the pointer exposure/recovery or typed
  SAFEARRAY byte-carrier provenance scope of this bead. The closure claim is
  deliberately limited to provenance and ownership lifetime; it does not claim
  that every public `VariantCore` construction path is already safe.
- Fresh-eyes review found a separate pre-existing safe-construction defect:
  public short union-field initialization can leave part of the eight-byte
  payload uninitialized before safe `data_bytes`, `Debug`, `Eq`, or wire reads.
  P0 successor `bd-59co.2.2.21` owns making partial initialization
  unrepresentable while preserving the 16-byte ABI. That issue is not evidence
  against the exposure/recovery implementation delivered here, and this bead
  takes no completion credit for it.
- Unsafe trusted decoding remains intentionally unsafe: using pointer bytes
  after their owner is dropped, or supplying fabricated non-null bytes, violates
  its documented precondition. The safe untrusted-byte decoder rejects all such
  carriers before provenance recovery.
- This slice does not claim `-Zmiri-strict-provenance` compatibility for an
  address-only ABI. Exposed Provenance is the explicit Rust contract for the
  required live in-process address round trip; the warnings remain visible in
  evidence rather than being suppressed.
- Transactional owning-record replacement remains owned by
  `bd-59co.2.2.16`. Unwind-safe borrowed BSTR/SAFEARRAY wrappers remain owned by
  `bd-59co.2.2.17`. This slice does not modify or claim either sibling outcome.
- Pointer-helper VBA `VarPtr`/`ObjPtr`, native callback-address, and external
  foreign-COM lifetime certification remain in their owning pointer/Windows
  lanes. They are not used as evidence for broader pointer or native-call
  completion here.
- The non-product VM3 foreign-object test double still contains the older
  first-field projection pattern. Support successor `bd-59co.2.2.22` owns its
  full-allocation provenance repair and sibling-fixture audit; it does not
  reopen the product runtime carrier result proved by this bead.
