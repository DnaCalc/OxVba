# Workset: Windows VBA 7.1 x64 Value Model Migration, Baseline Review, Old/New Matrix, and Final Report

Date: 2026-04-20
Owner: Codex
Status: in-progress

Reopen note, 2026-04-22:

The earlier closure path was too strong relative to the migration target fixed
in this workset. The current codebase has materially migrated `IUnknown`-style
object identity and has partially migrated `VARIANT`, but it has not yet
completed full internal native-layout alignment for `BSTR`, `SAFEARRAY`, or
broad struct / UDT layout. Epic and workset status were therefore reopened so
the tracker matches the actual implementation truth.

## 1. Purpose

Execute a full internal data-representation migration so that OxVba's internal
runtime data model matches current Windows VBA 7.1 64-bit internals as far as
the project can clean-room ascertain them from:

1. actual VBA running in Excel on Windows,
2. published Microsoft specifications,
3. existing OxVba behavior only as a lower-priority regression reference.

This workset is not an open exploration of alternative value models.

The migration decision is already made:

1. the repo at tag `pre-value-model-migration-2026-04-20` is the fixed
   reference implementation,
2. a new implementation will be built whose internal data representations match
   Windows VBA 7.1 x64 internals as far as we know and can ascertain,
3. any observable semantic divergence from Windows Excel/VBA is a bug,
4. the comparison questions are now limited to:
   - what observable differences exist between old and new,
   - what the memory effects are,
   - what the timing effects are,
   - what discretionary engineering decisions were required where the real
     implementation could not be fully observed.

## 2. Correctness Authority

For this migration, correctness authority is fixed and must be applied
consistently in tests, docs, and reporting:

1. actual VBA running in Excel on Windows,
2. published VBA / COM / Automation / ABI specifications,
3. old OxVba implementation.

Consequences:

1. the old implementation is a regression anchor, not the final source of
   truth,
2. if old OxVba behavior diverges from Excel/VBA/spec, that is a bug in old
   OxVba, not a reason to preserve the divergence,
3. the new implementation must not inherit old mismatches simply to preserve
   baseline behavior,
4. the final report must classify any old/new discrepancy against this
   authority hierarchy instead of treating old/new parity as self-justifying.

## 3. Baseline Reference

The migration baseline is already fixed:

1. tag: `pre-value-model-migration-2026-04-20`
2. baseline commit: the commit pointed to by that tag
3. baseline meaning:
   - reference build for old/new execution,
   - reference behavior for old/new comparison artifacts,
   - rollback anchor,
   - stable corpus anchor while the migration proceeds.

This baseline tag is not higher authority than Excel/spec. It is the reference
implementation snapshot only.

## 4. Baseline Review Findings

The baseline has now been reviewed in the runtime, COM, host, and
representation-sensitive test surfaces. The plan below is constrained by what
the old code actually does today.

### 4.1 Old runtime truth is semantic-first, not Windows-layout-first

Current baseline findings:

1. `crates/oxvba-runtime/src/bstr.rs` defines `BStr` as
   `pub struct BStr(pub String);`
2. `RuntimeValue::String` stores that wrapper directly in
   `crates/oxvba-runtime/src/runtime_value.rs`
3. `RuntimeValue` is the canonical internal execution carrier today and is
   semantic rather than a raw Windows wire-layout mirror
4. `ObjectHandle`, `BindingHandle`, and `DynLinkSymbol` are `repr(transparent)`
   wrappers over `i32`, so internal object identity is token/handle-oriented
   today rather than COM-interface-pointer-oriented
5. `crates/oxvba-runtime/src/variant.rs` defines a project-owned 16-byte
   compatibility `Variant` type, but it is only a bounded subset bridge and not
   the actual canonical runtime substrate
6. the baseline `Variant` bridge explicitly rejects important runtime shapes
   such as `RuntimeValue::String`, `RuntimeValue::ObjectHandle`, and
   `RuntimeValue::ArrayIntent`
7. `crates/oxvba-com/src/model.rs` mirrors the semantic carrier approach
   through `ComValue`, again showing that the current product architecture is
   semantic-core plus translation layers rather than raw Windows-layout core.

Interpretation:

1. the old implementation is well understood enough to describe accurately,
2. it is not already "almost there" internally for BSTR/VARIANT parity,
3. the migration is therefore a real substrate migration, not a light boundary
   cleanup.

### 4.2 Several Windows-looking truths are synthesized today

Current baseline findings:

1. `crates/oxvba-runtime/src/pointer_helpers.rs` synthesizes Windows-looking
   cells through `OwnedBstr`, `OwnedBstrCell`, and `OwnedVariant`
2. `VarPtr(String)` and `VarPtr(Variant)` observables are currently achieved in
   part by constructing compatibility cells on demand rather than by exposing
   the canonical runtime object directly
3. `crates/oxvba-com/src/windows_variant.rs` performs the real COM boundary
   translation to and from Windows `VARIANT`, `BSTR`, `SAFEARRAY`,
   `VT_DISPATCH`, and `VT_UNKNOWN`
4. event/callback transport in `crates/oxvba-com/src/windows_runtime_state.rs`
   and `crates/oxvba-com/src/windows_connection_point.rs` also sits on top of
   semantic carriers and translation seams.

Interpretation:

1. the old implementation already exposes many correct Windows-facing behaviors,
   but often by synthesis at helper and boundary seams,
2. one central migration question is where those synthetic projections disappear
   because the internal representation now already matches the observable truth,
3. another central question is where a semantic abstraction must still remain
   even after the migration, and therefore must be documented as a deliberate
   design choice rather than accidental legacy.

### 4.3 Baseline correctness is already reasonably well pinned at the observable boundary

The current baseline has strong coverage in several high-signal lanes:

1. `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`
   - exercises `StrPtr`, `VarPtr`, and `ObjPtr`
   - covers `VarPtr(String)` BSTR-cell exposure
   - covers `VarPtr(Variant)` for string, scalar, and decimal-sensitive cases
   - covers VM and JIT behavior
2. `crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs`
   - covers Windows declare string marshaling and writeback through real APIs
   - covers `StrPtr` writeback and UTF-8 / wide conversion lanes
   - covers VM and JIT behavior
3. `crates/oxvba-host/tests/com_client_end_to_end.rs`
4. `crates/oxvba-host/tests/com_client_registered_lane.rs`
5. `crates/oxvba-host/tests/com_early_project_end_to_end.rs`
6. `crates/oxvba-host/tests/imported_collection_newenum_regression.rs`
   - together these provide meaningful baseline coverage for `VT_BSTR`,
     `VT_DISPATCH`, `VT_UNKNOWN`, SAFEARRAY transport, early-bound interop, and
     `NewEnum`.

Interpretation:

1. the baseline implementation appears well understood and mostly correct in the
   observable string/pointer/native/COM lanes that matter most for this
   migration,
2. the old code should not be rewritten casually before the migration because
   it is already serving as a credible reference implementation,
3. the migration harness should reuse these strong lanes rather than inventing
   a new test philosophy.

### 4.4 Baseline gaps still need targeted baseline work

The review also exposed baseline gaps that should be repaired before or at the
front of the migration:

1. the current plan and code do not yet state baseline representation truth
   explicitly enough
2. the current benchmark scripts are too coarse for the migration question
3. event payload identity and broader struct / UDT / native-layout truth are not
   as well pinned as string/pointer/declare/COM dispatch lanes
4. current adapter seams are understood but not documented strongly enough as
   adapter seams
5. current old/new comparison execution is not yet a first-class artifact lane.

### 4.5 Baseline lifetime bug now explicitly recorded

Current baseline finding:

1. the repo's own spec and oracle evidence treat `Class_Terminate` as required
   observable teardown behavior
2. `Engine::create_class_instance` explicitly runs `Class_Initialize`, but the
   ordinary internal-class runtime does not yet show a general last-reference /
   scope-exit trigger that reliably invokes `Class_Terminate`
3. the COM server scaffold still carries an explicit future note to trigger
   `Class_Terminate` on final `Release`
4. the Excel oracle capture for `class_lifecycle_terminate_fail.bas` records a
   successful result where old OxVba errors
5. this is now recorded as `DIV-0005`, an old-OxVba bug rather than an allowed
   semantic alternative.

Interpretation:

1. lifetime/teardown-sensitive object behavior must be evaluated against
   Excel/spec first, not against the old implementation
2. the F lane should treat COM identity and lifetime as one problem, not two
   loosely coupled ones
3. the retained-wrapper migration target therefore uses `IUnknown` as the
   internal identity/lifetime anchor
4. the migrated runtime does not keep a separate integer `ObjectHandle` as the
   canonical object identity token
5. the canonical runtime object carrier becomes `ObjectRef`, backed by an
   `IUnknown`-implementing object/base structure
6. runtime reference counting for object lifetime must be driven through
   `IUnknown::AddRef` / `IUnknown::Release`.

## 5. Baseline Work Before the Migration

Minor baseline cleanup is justified, but only where it improves migration
clarity, testability, or evidence quality without changing intended behavior.

### 5.1 Allowed baseline-prep work

The baseline-prep lane should do the following:

1. document the current old representation truth explicitly in the workset and
   affected architecture/runtime docs
2. add or tighten baseline tests that prove the old seams we intend to migrate,
   especially:
   - the current old `BStr` substrate,
   - the current bounded old `Variant` bridge behavior,
   - the current synthetic `VarPtr(String)` / `VarPtr(Variant)` helper truth,
   - current `VT_UNKNOWN` / `VT_DISPATCH` identity behavior where observable
3. make existing projection seams easier to reason about, for example by
   centralizing naming or comments so code that constructs synthetic Windows
   helper cells is explicitly marked as projection logic
4. build the old/new harness, perf corpus, memory corpus, and report skeleton
   before the broad substrate rewrite
5. capture a baseline fact pack and baseline evidence artifacts from the fixed
   tag.

### 5.2 Disallowed baseline-prep work

The baseline-prep lane must not do the following:

1. partially migrate the internal value model under the guise of cleanup
2. rewrite major runtime/COM modules for style only
3. change semantic behavior except to fix a confirmed baseline bug against the
   authority hierarchy
4. create a permanent second canonical runtime just to support comparison.

### 5.3 Recommended baseline cleanup candidates

The review suggests the following narrow baseline cleanup candidates:

1. tighten documentation and comments around:
   - `crates/oxvba-runtime/src/bstr.rs`
   - `crates/oxvba-runtime/src/variant.rs`
   - `crates/oxvba-runtime/src/pointer_helpers.rs`
   - `crates/oxvba-com/src/model.rs`
   - `crates/oxvba-com/src/windows_variant.rs`
2. add explicit baseline tests that describe the old subset boundaries instead
   of leaving them implicit
3. add explicit baseline test anchors for event payload/object identity where
   the current coverage is weaker
4. add a comparison harness that runs the fixed baseline tag and the migration
   branch/head as two artifact-producing implementations.

### 5.4 Approved baseline cleanup scope and residual gap register

Approved baseline cleanup scope for this workset:

1. truth-surface clarification only
   - explicitly document the old checked-in representation in the workset and
     architecture docs
2. projection-seam labeling only
   - mark pointer-helper and COM translation seams as projections from the old
     semantic-first carrier rather than intrinsic runtime layout truth
3. bounded baseline tests only
   - add tests that pin old representation-sensitive subset boundaries that were
     still implicit
4. harness/report preparation only
   - create the old/new matrix, evidence roots, and report skeleton before the
     real substrate rewrite starts.

Explicitly not approved as baseline cleanup:

1. replacing the old string carrier
2. replacing the old value/Variant carrier
3. changing internal object/interface identity ownership
4. broad runtime/COM refactors done only for style or speculative cleanup.

Completed baseline cleanup items in the current lane:

1. the old checked-in value-model truth is now recorded in this workset and in
   `docs/ARCHITECTURE.md`
2. pointer-helper and COM projection seams are now marked explicitly in source
   comments
3. old runtime `Variant` subset-boundary tests now pin rejection of string,
   array-intent, object-handle, and binding-handle runtime shapes.

Residual baseline gaps that remain tracked rather than folded into cleanup:

1. the Windows VBA 7.1 x64 fact pack still needs to be published
   - owner: `vmm-b*`
2. the old/new correctness, perf, and memory harness still needs to be built
   - owner: `vmm-c*`
3. event payload/object identity rows still need stronger explicit matrix and
   evidence treatment
   - owner: `vmm-c6`, then `vmm-f*`
4. broader UDT/layout-sensitive closure still belongs to the later ABI/layout
   lane rather than baseline cleanup
   - owner: `vmm-g*`.

## 6. Migration Target and Execution Principles

The target is a Windows VBA 7.1 x64-style internal representation across all
platforms.

That means the internal representation should converge toward the Windows x64
VBA/COM model even on non-Windows builds, subject to clean-room ascertainment
and practical portability wrappers.

The migration target includes at least:

1. string / `BSTR`-style internal representation,
2. `Variant` / `VARIANT`-style internal representation,
3. COM interface identity modeled as COM-style binary interfaces
   (`IUnknown`/`IDispatch` and related interface-pointer truth) where relevant,
4. event payload transport and event object identity reviewed against that same
   data model,
5. struct / UDT / native layout implications,
6. pointer helper implications (`StrPtr`, `VarPtr`, `ObjPtr`),
7. host/COM/native marshaling implications.

Execution principles:

1. baseline behavior is compared from the fixed tag, not from a reinterpreted
   memory of how the old code used to work
2. a temporary in-repo old/new selector is allowed only if it accelerates
   comparison without becoming a second long-term runtime architecture
3. comparison artifacts must always label `old` and `new` explicitly
4. correctness proof comes before optimization claims
5. internal/boundary ownership doctrine from `OPERATIONS.md` still applies:
   layout alignment may be used as an implementation choice, but truth-surface
   ownership and crate responsibility must remain explicit.

### 6.1 Representation closure doctrine for this workset

For this workset, representation-family closure is controlled by the canonical
internal carrier truth, not only by boundary behavior or matrix evidence.

Rules:

1. a representation epic does not close merely because boundary-visible
   behavior is Windows-like or because the selected old/new matrix is green
2. for this workset, `BSTR` means the canonical internal string type is the
   actual BSTR representation, not a BSTR-shaped or UTF-16-compatible project
   carrier
3. for this workset, `VARIANT` means the canonical internal late-bound/general
   value used for `Dim x` is exactly the Windows/COM `VARIANT` representation,
   including real `BSTR`, `SAFEARRAY*`, and interface-pointer union payloads
4. if the canonical internal carrier is still semantic-first and Windows-like
   truth is only projected or materialized at pointer/COM/native seams, that
   family remains `in-progress`
5. a bounded subset can close only when the epic title and workset scope have
   been explicitly narrowed to that bounded subset; silent reinterpretation is
   not allowed
6. every still-open representation epic must end with an explicit
   representation-closure checklist bead
7. the final migration report cannot return to `final` status until those
   checklist beads are complete and the workset status is consistent with them.

### 6.2 Current intrinsic/projected/bounded grid

This grid is the current anti-drift truth surface for closure.

1. string / `BSTR`
   - intrinsic target:
     canonical runtime string storage itself is the BSTR representation
   - current truth:
     canonical `BStr` is now a pointer-sized owner of a raw BSTR payload;
     Windows builds allocate, clone, measure, and free through `SysAllocStringLen`,
     `SysStringLen`, and `SysFreeString`, while non-Windows builds retain the
     same length-prefix-plus-UTF-16-payload layout as a COM-compatible emulation
   - projected truth:
     real BSTR allocations are still materialized at pointer-helper and COM
     seams, and borrowed UTF-8 text is now a derived compatibility projection
     instead of carrier state
   - checklist result (`2026-04-23 correction`):
     - superseded:
       prior closure language treated BSTR-shaped storage as sufficient
     - implemented:
       `vmm-d7` delivered the internal BSTR carrier replacement; `OwnedBStrCore`
       is now only a compatibility snapshot/view
     - still open:
       `vmm-d8` must record the string/BSTR intrinsic closure checklist before
       the family can close
   - closure implication:
     `vmm-d7` is complete; `vmm-d8` remains open
2. `Variant` / `VARIANT`
   - intrinsic target:
      canonical runtime late-bound/general value carrier is exactly the
      Windows/COM `VARIANT` representation, including real `BSTR`,
      `SAFEARRAY*`, and interface-pointer payloads
   - current truth:
      canonical `Variant` is now back to the exact 16-byte carrier for scalar,
      string, and `ObjectRef`-backed object lanes, with clone/drop driven from
      the payload bytes themselves; canonical `SafeArray` now owns a real raw
      SAFEARRAY-style descriptor plus contiguous canonical typed payload
      storage, with intrinsic element-vartype preservation now landed for
      `VT_ARRAY | VT_VARIANT`, typed scalar lanes, `VT_BSTR`, `VT_DECIMAL`,
      `VT_DISPATCH`, and `VT_UNKNOWN`; `BindingHandle` is now treated
      explicitly as an internal non-VBA token outside the canonical `VARIANT`
      scope rather than as a missing Variant/SAFEARRAY carrier lane; this is
      still not sufficient because the general internal value model has not yet
      become the exact Windows/COM `VARIANT` representation everywhere;
      `Variant` now also owns `VT_ARRAY | VT_VARIANT` payloads as real
      `SAFEARRAY*` union values; VM registers now store normal VBA values as
      `Variant` slots rather than `RuntimeValue` slots; `ReDim` now preserves
      typed SAFEARRAY element vartypes and one-dimensional descriptor bounds;
      `For Each` iterator state and WithEvents binding state now also retain
      values as `RuntimeSlot` values rather than `RuntimeValue` values;
      however the semantic `RuntimeValue` projection and `ComValue` bridge
      remain open consolidation blockers
   - projected truth:
      full Windows `VARIANT` and typed-element `SAFEARRAY` truth is still
      materially realized at boundary/helper seams for array-sensitive lanes
   - checklist result (`2026-04-23 correction`):
     - superseded:
       `VARIANT_SAFEARRAY_INTRINSIC_CLOSURE_CHECKLIST_2026-04-23.md` accepted
       native-shaped carriers as closure evidence
     - still open:
       `Dim x` / general late-bound internal storage must be exactly
       Windows/COM `VARIANT`; array payloads must be real `SAFEARRAY*`
       payloads in the union slot
   - closure implication:
     `vmm-e6` and `vmm-e7` are reopened
3. COM interface identity
   - intrinsic target:
     canonical runtime object identity and lifetime are `IUnknown`-backed
   - current truth:
     `ObjectRef` and the runtime `IUnknown` substrate are materially landed
   - closure implication:
     this family may remain closed unless reopened by later scope expansion
4. struct / UDT / native layout
   - intrinsic target:
     either real internal/native layout parity is implemented for the scoped
     lane or the workset is explicitly narrowed
   - current truth:
     bounded non-boundary deterministic UDT subset only
   - projected/bounded truth:
     pointer/native lanes are partially reconciled, but broad struct-overlay
     and unconstrained UDT-byref parity remain bounded
   - closure implication:
     `vmm-g` remains open
5. final closure
   - intrinsic target:
     no family is reported as fully migrated while its intrinsic closure
     checklist remains open
   - closure implication:
     `vmm-h` and the workset remain open until the family checklist beads pass.

## 7. Representation Migration Scope and Actions

This section is the authoritative scope map for what changes, how it changes,
which baseline facts already matter, and what tests/docs must move with it.

### 7.1 String / `BSTR`

Baseline facts:

1. `BStr` is currently a thin Rust `String` wrapper
2. `RuntimeValue::String` stores that wrapper
3. Windows-looking BSTR cells are projected at helper and COM seams today.

Target truth:

1. internal string representation becomes BSTR
2. string identity, length, null/empty behavior, and pointer exposure line up
   with Windows/VBA expectations
3. helper or COM-boundary BSTR materialization is not accepted as the canonical
   internal representation.

Required migration actions:

1. define the new owned string carrier and its null/empty/len/pointer rules
2. rework `RuntimeValue::String` and any runtime helpers that assume UTF-8-owned
   `String`
3. update coercion, concatenation, comparison, slicing, and any other string
   operations that depend on storage shape
4. update VM/JIT helper code that assumes the old string carrier
5. reevaluate `StrPtr` and `VarPtr(String)` so the observable cell shape is a
   direct consequence of the new internal truth wherever honest
6. retain only the minimum necessary boundary adaptation for portability or
   unsupported edges
7. update architecture/runtime docs to describe the new string truth.

Expected code areas:

1. `crates/oxvba-runtime/src/bstr.rs`
2. `crates/oxvba-runtime/src/runtime_value.rs`
3. `crates/oxvba-runtime/src/coerce.rs`
4. `crates/oxvba-runtime/src/pointer_helpers.rs`
5. `crates/oxvba-vm/src/semantics.rs`
6. `crates/oxvba-jit/src/slot_abi.rs`
7. `crates/oxvba-jit/src/runtime_helpers.rs`
8. `crates/oxvba-hal/src/adapters/standard/*.rs`
9. `crates/oxvba-com/src/windows_variant.rs`
10. string-heavy host/conformance tests
11. `docs/ARCHITECTURE.md`

Primary baseline test anchors:

1. `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`
2. `crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs`

New matrix emphasis:

1. empty vs `vbNullString`
2. small/medium/long string workloads
3. many-string churn workloads
4. code-string workloads
5. BSTR boundary timing and allocation effects.

Intrinsic closure checklist for this family:

1. canonical runtime `BStr` is the BSTR representation, not `String`,
   BSTR-shaped UTF-16 storage, or a projection wrapper
2. canonical runtime string storage itself owns BSTR length, pointer,
   null/empty, and lifetime semantics
3. boundary code consumes or adapts the canonical BSTR representation rather
   than constructing the only real BSTR at the boundary
4. the family checklist bead records the final `implemented` / `projected` /
   `bounded` state explicitly before `vmm-d` can close.

### 7.2 `Variant` / `VARIANT`

Baseline facts:

1. `RuntimeValue` is the real runtime substrate today
2. `crates/oxvba-runtime/src/variant.rs` is a bounded 16-byte compatibility
   `Variant`
3. the old `Variant` bridge rejects strings, objects, and arrays
4. `crates/oxvba-com/src/windows_variant.rs` already performs the true Windows
   `VARIANT` boundary mapping.

Target truth:

1. OxVba's internal late-bound/general value container used for `Dim x` is
   exactly the Windows/COM `VARIANT` representation as defined by Windows
   headers and COM Automation specifications
2. subtype storage, width, alignment, reserved fields, decimal/currency/date
   handling, and pointer-bearing payloads match that `VARIANT`
3. string payloads are real `BSTR` pointers in the union slot
4. array payloads are real `SAFEARRAY*` pointers in the union slot
5. object/interface payloads are real COM-compatible interface pointers in the
   union slot
6. pointer-helper truth for `VarPtr(Variant)` becomes a direct consequence of
   the internal model wherever honest.

Required migration actions:

1. remove the separate semantic-first late-bound value carrier for `Dim x`
   storage, or make it a transparent wrapper over the exact internal
   Windows/COM `VARIANT`
2. define how strings, objects, arrays, decimal, currency, and date live in the
   new value carrier
3. reconcile `RuntimeValue`, `Variant`, and `ComValue` so the new internal
   model is not just another adapter layer on top of the old one
4. update compat-slot assumptions and any VM/JIT helper code coupled to old
   payload shapes
5. update pointer-helper `VARIANT` exposure
6. preserve correct Windows boundary behavior in `windows_variant.rs`
7. update docs and evidence to show which old subset limitations disappeared.

Expected code areas:

1. `crates/oxvba-runtime/src/runtime_value.rs`
2. `crates/oxvba-runtime/src/variant.rs`
3. `crates/oxvba-runtime/src/pointer_helpers.rs`
4. `crates/oxvba-runtime/src/lib.rs`
5. `crates/oxvba-com/src/model.rs`
6. `crates/oxvba-com/src/windows_variant.rs`
7. `crates/oxvba-com/src/miri_variant_mock.rs`
8. `crates/oxvba-hal/src/conformance.rs`
9. VM/JIT slot and helper code that assumes old semantic payload shapes
10. docs describing runtime value truth.

Primary baseline test anchors:

1. `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`
2. COM client and early-bound host tests

New matrix emphasis:

1. `VarPtr(Variant)` for string, numeric, decimal, object, and array-sensitive
   cases
2. old subset-limit tests converted into new full-carrier tests
3. memory/layout observations for the new value carrier.

Intrinsic closure checklist for this family:

1. canonical runtime late-bound/general value storage is exactly Windows/COM
   `VARIANT`
2. string, object, and array payload lanes are represented in the actual
   `VARIANT` union payload as real `BSTR`, interface pointer, and `SAFEARRAY*`
   values
3. `SAFEARRAY` interaction is no longer only a boundary truth for the scoped
   internal array lane, unless the workset is explicitly narrowed
4. the family checklist bead records the final `implemented` / `projected` /
   `bounded` state explicitly before `vmm-e` can close.

### 7.3 COM interface identity (`IUnknown`, `IDispatch`, related interfaces)

Baseline facts:

1. internal identity is currently token/handle-oriented in important places
2. boundary conversion already reaches real COM interface pointers where needed
3. `ComValue` and runtime values currently preserve semantic identity rather
   than raw interface-pointer truth internally.

Target truth:

1. internal object/interface identity aligns with Windows VBA 7.1 x64 COM
   binary-interface truth as far as we can honestly claim
2. any retained abstraction or indirection is documented as a deliberate
   project decision rather than accidental legacy
3. the canonical runtime object carrier is `ObjectRef`, not a standalone
   integer handle token
4. the runtime object/base structure for object-valued identities implements
   `IUnknown`
5. runtime object lifetime/refcounting is driven by `IUnknown::AddRef` /
   `IUnknown::Release`
6. retained native COM identity/lifetime truth is anchored on canonical
   `IUnknown` identity, with `IDispatch` retained as the Automation invocation
   surface where applicable
7. `VT_UNKNOWN` and `VT_DISPATCH` behavior is reevaluated under the new
   representation.

Required migration actions:

1. replace token-only runtime object identity with canonical `ObjectRef`
2. make the resolved runtime object/base structure `IUnknown`-implementing
3. route runtime object lifetime/refcounting through `IUnknown::AddRef` /
   `IUnknown::Release`
4. update retained runtime/com state so identity dedup and lifetime ownership
   are keyed off canonical retained `IUnknown` truth rather than only retained
   `IDispatch*`
5. preserve lifetime, ownership, and safety semantics across callbacks and
   boundary calls
6. retest `VT_UNKNOWN`, `VT_DISPATCH`, and `ObjPtr`
7. document all places where the project still chooses a wrapper rather than a
   raw pointer as the primary carrier.

Expected code areas:

1. `crates/oxvba-runtime/src/runtime_value.rs`
2. `crates/oxvba-runtime/src/variant.rs`
3. `crates/oxvba-vm/src/interpreter.rs`
4. `crates/oxvba-jit/src/slot_abi.rs`
5. `crates/oxvba-com/src/model.rs`
6. `crates/oxvba-com/src/dynamic_object.rs`
7. `crates/oxvba-com/src/runtime_state.rs`
8. `crates/oxvba-com/src/windows_runtime_state.rs`
9. `crates/oxvba-com/src/windows_variant.rs`
10. `crates/oxvba-com/src/windows_client.rs`
11. `crates/oxvba-com/src/windows_invoke.rs`
12. `crates/oxvba-com/src/windows_connection_point.rs`
13. COM host tests
14. architecture/runtime/interop docs.

Primary baseline test anchors:

1. `crates/oxvba-host/tests/com_client_end_to_end.rs`
2. `crates/oxvba-host/tests/com_client_registered_lane.rs`
3. `crates/oxvba-host/tests/com_early_project_end_to_end.rs`
4. `crates/oxvba-host/tests/imported_collection_newenum_regression.rs`
5. `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`

New matrix emphasis:

1. object identity stability
2. `VT_UNKNOWN` / `VT_DISPATCH` roundtrips
3. callback and event object lifetime behavior.

### 7.4 Event model

Baseline facts:

1. event payloads currently travel through `ComValue` and related callback
   machinery
2. the event system already depends on the semantic-carrier design
3. current event behavior may stay correct even if only limited internal shape
   changes are required.

Target truth:

1. event payload transport is reviewed under the new internal representation
2. callback object identity and payload storage line up with the migrated value
   model
3. if the event model does not materially benefit from a direct representation
   change, that becomes an explicit discretionary decision rather than an
   implicit omission.

Required migration actions:

1. audit current callback payload storage and identity flow
2. decide what changes materially improve VBA/COM parity
3. add missing baseline tests if event identity or payload truth is currently
   under-asserted
4. document any no-change decision with evidence.

Expected code areas:

1. `crates/oxvba-com/src/windows_connection_point.rs`
2. `crates/oxvba-com/src/windows_runtime_state.rs`
3. `crates/oxvba-com/src/model.rs`
4. `crates/oxvba-com/src/dynamic_object.rs`
5. event-related host tests and docs.

Primary baseline test anchors:

1. existing COM event tests
2. any callback-related host fixtures
3. new baseline tests added in the baseline-prep lane if current coverage is too
   weak.

### 7.5 Struct layout / UDT / native ABI

Baseline facts:

1. OxVba has a bounded UDT/runtime subset
2. native declare and pointer-helper behavior already expose some boundary
   truths
3. broader struct-layout truth is not yet centralized enough for this
   migration.

Target truth:

1. the migration documents and implements how Windows VBA 7.1 x64-style
   internal layout interacts with UDTs and native ABI-sensitive shapes
2. `VarPtr`, `StrPtr`, `ObjPtr`, native writeback, and struct container truth
   are all retested
3. deterministic unsupported cases remain explicit where broader truth still
   cannot be honestly claimed.

Required migration actions:

1. define the new layout interaction between value carriers and UDT/native
   writeback surfaces
2. retest pointer-exposed and writeback-sensitive cases
3. add explicit documentation for unsupported or intentionally bounded cases
4. record any still-open evidence gaps that limit stronger claims.

Expected code areas:

1. `crates/oxvba-runtime/src/pointer_helpers.rs`
2. `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`
3. `crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs`
4. native declare / dynlink runtime and HAL files
5. relevant UDT tests and type/runtime docs.

Intrinsic closure checklist for this family:

1. if the epic title continues to claim struct / UDT / native-layout
   reconciliation, broad native-layout truth cannot be replaced silently by a
   bounded compiler/runtime subset
2. either the scoped native-layout/UDT internal truth is implemented, or the
   workset text is explicitly narrowed with the user's approval
3. any still-bounded native ABI areas are listed as bounded rather than being
   treated as implicitly closed
4. the family checklist bead records the final `implemented` / `projected` /
   `bounded` state explicitly before `vmm-g` can close.

## 8. Windows VBA 7.1 x64 Fact Pack

The migration direction is fixed, but implementation still needs one explicit
fact pack that records what is actually known about Windows VBA 7.1 x64
internals.

This is not an open design-decision lane. It is a bounded documentation input
to implementation.

The fact pack must record, for each migrated type family:

1. ascertained size
2. ascertained alignment
3. owned/borrowed/null/empty sentinel behavior
4. observable pointer semantics
5. copy/writeback semantics
6. authoritative evidence source:
   - Excel/VBA oracle,
   - published specification,
   - project decision where observation/spec is insufficient.

Required fact-pack families:

1. string / BSTR
2. Variant / VARIANT
3. interface pointers / `IUnknown` / `IDispatch`
4. SAFEARRAY interaction
5. event payload carrier implications
6. pointer-helper observable cells
7. native ABI / struct layout implications.

The fact pack must also record:

1. what is known about the old representation at the same seam
2. what the intended migration delta is
3. whether the seam is expected to become direct truth, retained wrapper, or
   documented adapter.

## 9. Old/New Testing Matrix

The testing matrix is mandatory and must be run against both:

1. the fixed baseline tag implementation (`old`)
2. the migrated head implementation (`new`).

Where a temporary in-repo old/new switch helps development, it is optional and
secondary. The canonical comparison remains fixed baseline tag vs migrated head.

### 9.1 Matrix dimensions

Every matrix row must identify:

1. representation:
   - `old`
   - `new`
2. execution backend:
   - VM
   - JIT
   - host-backed
   - COM-backed
3. authority comparison:
   - old/new only
   - spec-backed
   - Excel oracle-backed
4. artifact class:
   - correctness
   - performance
   - memory.

### 9.2 Required correctness families

The minimum correctness matrix families are:

1. core runtime string semantics
   - empty vs `vbNullString`
   - concat
   - slices
   - compare/search
   - replace/split/join
2. pointer-helper truth
   - `StrPtr`
   - `VarPtr(String)`
   - `VarPtr(Variant)` for string/scalar/decimal/object-sensitive cases
   - `ObjPtr`
3. native declare / dynlink truth
   - string in
   - string writeback
   - buffer writeback
   - currency/date/numeric writeback
4. COM late-bound client truth
   - scalar `VT_*`
   - `VT_BSTR`
   - `VT_DISPATCH`
   - `VT_UNKNOWN`
   - SAFEARRAY scalar and BSTR cases
   - SAFEARRAY nested object/variant cases
5. COM early-bound/imported reference truth
   - object-valued members
   - `NewEnum`
   - imported object/variant/property behavior
6. COM event truth
   - callback payload transport
   - subscription/unsubscription lifecycle
   - object/string payload cases
7. UDT / struct / pointer-helper ABI truth
8. VM/JIT parity on all new migrated representations.

### 9.3 Required performance families

The performance matrix must include at least:

1. small-string operations
   - empty
   - 1-char
   - 8-char
   - 15-char
   - 31-char
2. medium-string operations
   - 64-char
   - 128-char
   - 256-char
3. long-string operations
   - 1 KiB
   - 4 KiB
   - 16 KiB
   - 64 KiB
4. many-string workloads
   - many short literals
   - repeated allocation churn
   - split/join arrays
   - dictionary or collection-like string churn where relevant
5. code-string workloads
   - module-sized code text
   - project metadata string churn
   - compile/load workflows with many identifiers and path strings
6. boundary workloads
   - COM `VT_BSTR` arg/result
   - SAFEARRAY<BSTR>
   - native declare string marshaling
   - pointer-helper string cell exposure.

Each perf row must record:

1. old/new representation
2. workload id
3. string-size family where relevant
4. mean/min/max time
5. iteration count
6. build mode
7. environment metadata.

### 9.4 Required memory families

The memory matrix must record at least:

1. per-value/container size and alignment observations for old/new
2. peak and steady-state process memory for representative workloads
3. allocation-sensitive string churn workloads
4. COM string/variant heavy workloads
5. any measured reductions in temporary boundary allocations.

### 9.5 Baseline-driven matrix additions from the code review

The baseline review adds these explicit matrix needs:

1. old `Variant` subset-boundary tests:
   - string rejection
   - object rejection
   - array rejection
2. old/new comparison of synthetic-vs-direct pointer-helper truth:
   - `VarPtr(String)` cell construction
   - `VarPtr(Variant)` cell construction
3. explicit event payload/object identity rows
4. explicit struct / UDT / native writeback rows where current evidence is
   thinner than COM/string lanes
5. explicit "temporary boundary allocation count / rate" observations where
   practical.

## 10. Sequence of Implementation

The sequence is fixed. Do not start by rewriting the internal model blindly.

### Phase 0. Baseline review lock, cleanup, and evidence seed

This phase happens first.

Deliver:

1. this workset updated with the reviewed baseline truth
2. narrow baseline cleanup/refactor tasks selected and bounded
3. baseline-specific tests added where current representation truth is still too
   implicit
4. baseline evidence/artifact skeleton created
5. top-level bead layout created for the workset.

### Phase 1. Fact pack and oracle capture

Deliver:

1. Windows VBA 7.1 x64 fact pack for strings, variants, interfaces, events, and
   layout-sensitive lanes
2. explicit evidence references for each family
3. open discretionary-decision list seeded only where evidence/spec is
   genuinely incomplete.

### Phase 2. Extend the test and measurement harness first

This phase must happen before broad representation changes.

Current repo substrate to build on:

1. correctness baselines already exist in:
   - `scripts/run-conformance.ps1`
   - `scripts/run-project-integration-suite.ps1`
   - `scripts/run-matrix.ps1`
   - `scripts/run-com-early-conformance.ps1`
2. focused representation-sensitive anchors already exist in:
   - `crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs`
   - `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`
   - `crates/oxvba-host/tests/com_client_end_to_end.rs`
   - `crates/oxvba-host/tests/com_client_registered_lane.rs`
   - `crates/oxvba-host/tests/com_early_project_end_to_end.rs`
3. coarse performance baselines already exist in:
   - `scripts/run-bench.ps1`
   - `scripts/run-com-early-perf.ps1`
4. the migration harness must wrap and extend those assets rather than inventing a
   disconnected parallel test world.

Deliver:

1. deterministic old/new execution runners using the fixed baseline tag and
   migrated head
2. expanded correctness corpus for migrated surfaces
3. expanded performance corpus, especially string-size/string-count workloads
4. memory-usage measurement hooks
5. deterministic comparison scripts
6. report skeleton populated with artifact locations.

Rationale:

1. the migration must be driven by a stronger corpus than the current one
2. old/new behavior must be directly comparable from the start
3. the codebase is too large to rely on one agent session's local context.

Harness-shape decisions for this repo:

1. correctness comparison should be orchestrated by a new top-level old/new
   runner that invokes the existing correctness substrate plus focused
   representation-sensitive cargo tests
2. performance comparison should reuse the existing benchmark conventions and add
   a dedicated string workload runner instead of overloading the profile bench
   lane
3. memory comparison should capture both value/container observations
   (`size_of`, alignment, pointer-cell-sensitive snapshots) and process-level
   working-set observations for representative workloads
4. canonical artifact roots should live under
   `docs/evidence/value_model_migration/` with stable `baseline/`, `candidate/`,
   `comparison/`, and `report_inputs/` subtrees
5. new harness scripts should be introduced as explicit migration-owned surfaces:
   - `scripts/run-value-model-correctness.ps1`
   - `scripts/run-value-model-string-perf.ps1`
   - `scripts/run-value-model-memory.ps1`
   - `scripts/compare-value-model-results.ps1`
6. the fixed baseline tag `pre-value-model-migration-2026-04-20` remains the
   old lane authority for all paired runs unless a later workset explicitly
   supersedes it.

### Phase 3. Representation migration implementation

Only after the harness exists:

1. migrate string/BSTR substrate and string-sensitive runtime users
2. migrate Variant/value-container substrate
3. reconcile interface identity under the new model
4. update event payload and callback storage as required
5. update pointer-helper and native ABI-sensitive logic
6. repair all downstream VM/JIT/host/COM assumptions
7. update docs continuously as truth changes.

### Phase 4. Full old/new matrix execution

Run:

1. baseline old implementation from the fixed tag
2. migrated new implementation from head

against the full matrix and produce paired outputs.

### Phase 5. Final truth and report

Produce the required migration report described in section 12.

## 11. Full Bead Execution Plan

This section is the second-pass bead plan for the whole migration. It is meant
to be the full execution graph rooted in this workset, not just an epic index.
It therefore names:

1. the intended bead hierarchy,
2. the bead ids to use in planning,
3. the dependency order,
4. the priority bands that drive the ready queue,
5. the conditional rollout beads reserved for areas where one more bounded
   research/experiment pass may be needed before the exact child structure is
   honest.

### 11.1 Planning Conventions

Bead id scheme:

1. workset root parent: `vmm-root`
2. epic parents: `vmm-a` through `vmm-h`
3. child beads: `vmm-a0`, `vmm-a1`, ... `vmm-h4`
4. rollout beads use the `0` suffix by default
5. conditional sub-rollout beads use the next available id inside the same epic.

Tracker note:

1. actual bead-graph root in `.beads/` is `bd-t8rr`
2. the `vmm-*` ids in this section are the logical plan ids for the workset
3. the actual created tracker ids are the `bd-t8rr.*` subtree rooted at
   `bd-t8rr`.

Priority bands:

1. `P0`
   - current critical-path work
   - should dominate the ready queue until the migration substrate is ready
2. `P1`
   - primary implementation lane once required upstream work is complete
3. `P2`
   - secondary or parallel lane that can proceed once a parent dependency is
     satisfied
4. `P3`
   - conditional rollout, final synthesis, or non-blocking tightening work.

Dependency interpretation:

1. parent beads are summary/closure owners and are blocked on all required child
   beads in their epic
2. support beads do not substitute for delivery closure in capability epics
3. if a rollout bead or research bead reveals missing work, the discovered work
   becomes new child beads before the lane can honestly close
4. where a lane is intentionally marked with a later conditional rollout bead,
   that means one more bounded discovery pass is expected before exact leaf work
   can be trusted.

### 11.2 Root Hierarchy and Epic Order

Root parent:

1. `vmm-root`
   - kind: parent summary bead
   - close condition:
     - the terminal condition in section 14 is satisfied
   - child epic parents:
     - `vmm-a`
     - `vmm-b`
     - `vmm-c`
     - `vmm-d`
     - `vmm-e`
     - `vmm-f`
     - `vmm-g`
     - `vmm-h`.

Epic parents:

1. `vmm-a`
   - title: baseline truth lock and cleanup
   - priority: `P0`
   - depends on: none
2. `vmm-b`
   - title: Windows VBA 7.1 x64 fact pack
   - priority: `P0`
   - depends on: `vmm-a1`
3. `vmm-c`
   - title: old/new matrix and evidence harness
   - priority: `P0`
   - depends on: `vmm-a4`, `vmm-b4`
4. `vmm-d`
   - title: string/BSTR migration
   - priority: `P1`
   - depends on: `vmm-a5`, `vmm-b4`, `vmm-c7`
5. `vmm-e`
   - title: Variant/VARIANT migration
   - priority: `P1`
   - depends on: `vmm-d6`, `vmm-b4`, `vmm-c7`
6. `vmm-f`
   - title: interface identity and COM event transport
   - priority: `P1`
   - depends on: `vmm-e5`, `vmm-b4`, `vmm-c7`
7. `vmm-g`
   - title: struct / UDT / native ABI / pointer-helper reconciliation
   - priority: `P1`
   - depends on: `vmm-e5`, `vmm-b4`, `vmm-c7`
8. `vmm-h`
   - title: final matrix, docs, and report
   - priority: `P1`
   - depends on: `vmm-d6`, `vmm-e5`, `vmm-f6`, `vmm-g5`.

### 11.3 Ready-Queue Intent and Dependency Waves

This is the intended work sequence for the bead runner.

Wave 0:

1. `vmm-a0`

Wave 1:

1. `vmm-a1`

Wave 2:

1. `vmm-a2`
2. `vmm-a3`
3. `vmm-b0`

Wave 3:

1. `vmm-a4`
2. `vmm-b1`
3. `vmm-b2`
4. `vmm-b3`

Wave 4:

1. `vmm-a5`
2. `vmm-b4`

Wave 5:

1. `vmm-c0`
2. `vmm-c1`
3. `vmm-c2`
4. `vmm-c3`
5. `vmm-c4`
6. `vmm-c5`
7. `vmm-c6`
8. `vmm-c7`

Wave 6:

1. `vmm-d0`
2. `vmm-d1`
3. `vmm-d2`
4. `vmm-d3`
5. `vmm-d4`
6. `vmm-d5`
7. `vmm-d6`

Wave 7:

1. `vmm-e0`
2. `vmm-e1`
3. `vmm-e2`
4. `vmm-e3`
5. `vmm-e4`
6. `vmm-e5`

Wave 8:

1. `vmm-f0`
2. `vmm-f1`
3. `vmm-f2`
4. `vmm-f3`
5. `vmm-f4`
6. `vmm-f5`
7. `vmm-f6`
8. `vmm-g0`
9. `vmm-g1`
10. `vmm-g2`
11. `vmm-g3`
12. `vmm-g4`
13. `vmm-g5`

Wave 9:

1. `vmm-h0`
2. `vmm-h1`
3. `vmm-h2`
4. `vmm-h3`
5. `vmm-h4`

Parallelism note:

1. `vmm-b1`, `vmm-b2`, and `vmm-b3` may proceed in parallel after `vmm-b0`
2. `vmm-c2`, `vmm-c3`, `vmm-c4`, and `vmm-c5` may proceed in parallel after
   `vmm-c1`
3. `vmm-d3`, `vmm-d4`, and `vmm-d5` may proceed in parallel after `vmm-d2`
   where code ownership remains non-conflicting
4. `vmm-f*` and `vmm-g*` may proceed in parallel after `vmm-e5` except where
   `ObjPtr` or shared pointer-helper substrate work creates a direct dependency.

### 11.4 Epic A Bead Set: Baseline Truth Lock and Cleanup

Parent:

1. `vmm-a`
   - close condition:
     - baseline truth is documented,
     - baseline cleanup is bounded and executed,
     - missing baseline tests are landed or explicitly split into tracked
       follow-up beads.

Child beads:

1. `vmm-a0`
   - kind: `support`
   - priority: `P0`
   - depends on: none
   - title: `Roll out baseline truth lock and cleanup child beads`
   - outcome:
     - create the executable child bead set for the baseline lane
   - completion evidence:
     - epic `vmm-a` has explicit child beads and a believable ready path
2. `vmm-a1`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-a0`
   - title: `Publish baseline representation findings for old BStr, Variant, and handle carriers`
   - outcome:
     - old representation truth is documented with code references
   - completion evidence:
     - this workset and affected architecture docs reflect the actual old model
3. `vmm-a2`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-a1`
   - title: `Mark current synthetic pointer-helper and COM projection seams explicitly`
   - outcome:
     - old projection seams are labeled/documented as projection logic
   - completion evidence:
     - affected docs/comments/spec notes make the seam ownership explicit
4. `vmm-a3`
   - kind: `delivery`
   - priority: `P0`
   - depends on: `vmm-a1`
   - title: `Add missing baseline tests for old representation-sensitive seams`
   - outcome:
     - baseline subset boundaries and weaker event/layout lanes are pinned
   - completion evidence:
     - new or tightened baseline tests land and pass
     - matrix rows for old-only truth are explicit
5. `vmm-a4`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-a2`, `vmm-a3`
   - title: `Publish bounded baseline cleanup scope and residual gap register`
   - outcome:
     - the allowed cleanup/refactor pass is bounded explicitly
   - completion evidence:
     - accepted cleanup items and disallowed work are recorded
6. `vmm-a5`
   - kind: `delivery`
   - priority: `P0`
   - depends on: `vmm-a4`
   - title: `Execute approved baseline cleanup and baseline truth tightening`
   - outcome:
     - narrow baseline cleanup lands without changing intended semantics
   - completion evidence:
     - approved cleanup items are implemented
     - affected baseline tests still pass.

### 11.5 Epic B Bead Set: Windows VBA 7.1 x64 Fact Pack

Parent:

1. `vmm-b`
   - close condition:
     - fact pack families exist for all scoped type groups
     - evidence provenance and discretionary decision seeds are recorded.

Child beads:

1. `vmm-b0`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-a1`
   - title: `Roll out Windows VBA 7.1 x64 fact pack child beads`
   - outcome:
     - create the executable child bead set for the fact-pack lane
   - completion evidence:
     - all fact-pack families have explicit owners
2. `vmm-b1`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-b0`
   - title: `Publish BSTR and string-pointer fact pack`
   - outcome:
     - string/BSTR observable truth is documented
   - completion evidence:
     - size/alignment/null-empty/pointer semantics and sources are recorded
3. `vmm-b2`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-b0`
   - title: `Publish VARIANT and SAFEARRAY fact pack`
   - outcome:
     - Variant/SAFEARRAY truth is documented
   - completion evidence:
     - layout and carrier facts are recorded with provenance
4. `vmm-b3`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-b0`
   - title: `Publish interface identity, event payload, and layout-sensitive fact pack`
   - outcome:
     - interface/event/layout-sensitive facts are documented
   - completion evidence:
     - evidence-backed fact entries exist for each family
5. `vmm-b4`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-b1`, `vmm-b2`, `vmm-b3`
   - title: `Publish fact-pack consolidation and initial discretionary decision register`
   - outcome:
     - the fact pack is consolidated into a usable migration input
   - completion evidence:
     - open discretionary decisions are listed with evidence basis and revisit
       trigger.

### 11.6 Epic C Bead Set: Old/New Matrix and Evidence Harness

Parent:

1. `vmm-c`
   - close condition:
     - old/new correctness, perf, and memory lanes exist
     - the canonical migration matrix exists
     - baseline artifacts from the fixed tag are captured.

Child beads:

1. `vmm-c0`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-a4`, `vmm-b4`
   - title: `Roll out old/new matrix and evidence harness child beads`
   - outcome:
      - refresh the executable child bead set so it is grounded in the current
        repo harness substrate, script surfaces, and evidence roots
    - completion evidence:
      - correctness/perf/memory child beads exist explicitly and name the
        current scripts/tests/artifact roots they are expected to extend
2. `vmm-c1`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-c0`
   - title: `Publish canonical old/new migration matrix and traceability anchors`
   - outcome:
     - matrix rows exist for correctness, performance, and memory
   - completion evidence:
     - the matrix identifies authority, artifact class, and test anchors
3. `vmm-c2`
   - kind: `delivery`
   - priority: `P0`
   - depends on: `vmm-c1`
   - title: `Add deterministic old/new correctness runners against baseline tag and head`
   - outcome:
      - paired correctness artifacts can be emitted for old and new by wrapping
        the current correctness substrate (`run-conformance`, project
        integration, matrix, COM early conformance, and focused host tests)
    - completion evidence:
      - runnable scripts exist and emit labeled old/new artifacts with baseline
        tag vs head provenance in stable artifact directories
4. `vmm-c3`
   - kind: `delivery`
   - priority: `P0`
   - depends on: `vmm-c1`
   - title: `Add string-focused old/new perf runner`
   - outcome:
      - string-size and string-churn timing corpus exists beside the existing
        profile and COM early perf lanes
    - completion evidence:
      - small/medium/long/many/code string workloads produce paired artifacts
        with comparable baseline/head labeling
5. `vmm-c4`
   - kind: `delivery`
   - priority: `P0`
   - depends on: `vmm-c1`
   - title: `Add old/new memory measurement runner`
   - outcome:
      - old/new memory artifacts can be produced consistently for
        representation-sensitive values plus process-level workloads
    - completion evidence:
      - size/alignment/process-memory artifacts are emitted and indexed under
        the migration evidence root
6. `vmm-c5`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-c1`
   - title: `Publish evidence directory skeleton and final-report input index`
   - outcome:
      - evidence roots and report-input skeleton exist under
        `docs/evidence/value_model_migration/`
    - completion evidence:
      - artifact paths are documented, stable, and align with the new harness
        scripts
7. `vmm-c6`
   - kind: `delivery`
   - priority: `P0`
   - depends on: `vmm-c2`, `vmm-c5`
   - title: `Expand correctness corpus for under-covered event and layout-sensitive rows`
   - outcome:
      - under-covered migration rows have test anchors before the rewrite,
        especially event payload, pointer-helper, native string, and
        layout-sensitive rows
    - completion evidence:
      - missing event/layout-sensitive coverage lands and is referenced by the
        matrix
8. `vmm-c7`
   - kind: `support`
   - priority: `P0`
   - depends on: `vmm-c2`, `vmm-c3`, `vmm-c4`, `vmm-c5`
   - title: `Capture baseline old artifacts from the fixed tag`
   - outcome:
      - the fixed baseline tag has initial correctness/perf/memory artifacts
        captured through the same migration harness that head will use
    - completion evidence:
      - baseline old artifacts are generated and indexed.

### 11.7 Epic D Bead Set: String/BSTR Migration

Parent:

1. `vmm-d`
   - close condition:
     - internal string type is the actual BSTR representation
     - string correctness/perf/memory rows are reconciled
     - remaining string decisions are explicitly documented
     - the intrinsic string/BSTR closure checklist bead is complete.

Child beads:

1. `vmm-d0`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-a5`, `vmm-b4`, `vmm-c7`
   - title: `Roll out string/BSTR migration child beads`
   - outcome:
     - create the executable child bead set for string migration
   - completion evidence:
     - carrier, runtime-user, pointer-helper, boundary, and validation beads
       exist explicitly
2. `vmm-d1`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-d0`, `vmm-b1`
   - title: `Implement Windows-style owned string carrier core`
   - outcome:
     - the new internal string substrate exists
   - completion evidence:
     - core carrier code and carrier-local tests land
3. `vmm-d2`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-d1`
   - title: `Migrate runtime string operations and coercion paths`
   - outcome:
     - runtime string semantics move onto the new carrier
   - completion evidence:
     - affected runtime and conformance tests pass
4. `vmm-d3`
   - kind: `delivery`
   - priority: `P2`
   - depends on: `vmm-d2`
   - title: `Migrate VM, JIT, and HAL string assumptions`
   - outcome:
     - execution engines no longer assume the old UTF-8-owned string shape
   - completion evidence:
     - VM/JIT string-sensitive tests pass
5. `vmm-d4`
   - kind: `delivery`
   - priority: `P2`
   - depends on: `vmm-d2`
   - title: `Reconcile pointer-helper and native declare string surfaces`
   - outcome:
     - `StrPtr` and `VarPtr(String)` truth aligns with the new carrier
   - completion evidence:
     - pointer-helper and declare string tests pass
6. `vmm-d5`
   - kind: `delivery`
   - priority: `P2`
   - depends on: `vmm-d2`
   - title: `Reconcile COM BSTR translation and boundary allocation behavior`
   - outcome:
     - BSTR transport is correct and temporary boundary allocation behavior is
       reevaluated
   - completion evidence:
     - BSTR-sensitive COM tests and allocation observations are updated
7. `vmm-d6`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-d3`, `vmm-d4`, `vmm-d5`
   - title: `Run and reconcile string old/new correctness, perf, and memory matrix`
   - outcome:
     - string migration is validated as a whole
   - completion evidence:
     - string matrix rows are updated
     - remaining divergences are classified against the authority hierarchy.
8. `vmm-d7`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-d3`, `vmm-d4`, `vmm-d5`, `vmm-d6`
   - title: `Complete intrinsic internal BSTR carrier migration`
   - outcome:
     - the canonical runtime string carrier itself becomes BSTR rather than
       remaining `String`, BSTR-shaped storage, or boundary projection
  - current staged progress:
    - stage 1 landed: `BStr` is now opaque across production code and test
      code, so the old `pub String` tuple shape is no longer a cross-repo API
      dependency
    - stage 2 landed: `BStr` now stores a UTF-16/BSTR-shaped owned core as its
      internal source of truth, while retaining a cached UTF-8 view to keep the
      now-stable API usable during the remaining migration
    - stage 3 landed: canonical `Variant` string payloads no longer carry a
      second independent `OwnedBStrCore`; they reuse the intrinsic `BStr`
      carrier instead of reintroducing duplicated string-core truth one layer up
    - stage 4 landed: Windows pointer-helper BSTR allocation now uses
      length-aware `SysAllocStringLen` from the canonical `BStr` core, and the
      runtime test suite now proves embedded-NUL payload preservation for the
      `VarPtr(String)` BSTR-cell path instead of relying on NUL-terminated
      reconstruction
    - stage 5 landed: `BStr` no longer stores a permanently duplicated UTF-8
      payload beside the UTF-16/BSTR-shaped core; UTF-8 text is now a lazy
      compatibility cache derived from the intrinsic core rather than a second
      always-live carrier field
    - stage 6 landed: the compatibility cache itself has now been removed, and
      the repo is being driven onto a core-only `BStr` carrier so any remaining
      borrowed UTF-8 assumptions fail at compile time instead of hiding behind a
      second runtime string payload
    - a bounded paired memory refresh against the stage-5 candidate showed why
      that step was necessary: `BStr` had inflated to `56` bytes and `Variant`
      to `80` bytes while the always-live/lazy-cache hybrid was still in place
    - stage 6 still does not satisfy this bead; the 2026-04-23 correction
      clarifies that BSTR-shaped/core-only UTF-16 storage is progress, not
      closure, because the internal OxVba string type must become BSTR
    - stage 7 landed: `BStr` is a pointer-sized raw BSTR owner, using
      `SysAllocStringLen` / `SysStringLen` / `SysFreeString` on Windows and a
      COM-compatible raw layout emulation on non-Windows; `Variant`,
      `SafeArray`, pointer-helper, and COM string conversion paths clone from
      that canonical payload rather than rebuilding from boxed UTF-16 storage
   - completion evidence:
     - runtime string carrier truth is BSTR, and boundary/helper seams no
       longer create the only real BSTR representation
9. `vmm-d8`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-d7`
   - title: `Run string/BSTR intrinsic closure checklist`
   - outcome:
     - the final string-family `implemented` / `projected` / `bounded`
       classification is recorded before epic closure
   - completion evidence:
     - the workset and final report inputs explicitly state whether the string
       lane is intrinsically migrated or still bounded.

### 11.8 Epic E Bead Set: Variant/VARIANT Migration

Parent:

1. `vmm-e`
   - close condition:
     - the internal late-bound/general value substrate is exactly Windows/COM
       `VARIANT`
     - `VarPtr(Variant)` and COM variant lanes are reconciled
     - memory/layout observations are captured
     - the intrinsic Variant/SAFEARRAY closure checklist bead is complete.

Child beads:

1. `vmm-e0`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-d6`, `vmm-b4`, `vmm-c7`
   - title: `Roll out Variant/VARIANT migration child beads`
   - outcome:
     - create the executable child bead set for Variant migration
   - completion evidence:
     - carrier, boundary, pointer-helper, and validation beads exist explicitly
2. `vmm-e1`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-e0`, `vmm-b2`
   - title: `Implement new canonical Variant/value carrier`
   - outcome:
     - the new internal value/Variant substrate exists
   - completion evidence:
     - carrier implementation and core tests land
3. `vmm-e2`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-e1`
   - title: `Migrate RuntimeValue, ComValue, and compat-slot boundaries onto the new carrier`
   - outcome:
     - runtime and COM value carriers are reconciled around the new model
   - completion evidence:
     - runtime/COM bridging tests pass and old subset limits are removed or
       documented
4. `vmm-e3`
   - kind: `delivery`
   - priority: `P2`
   - depends on: `vmm-e2`
   - title: `Reconcile pointer-helper Variant exposure`
   - outcome:
     - `VarPtr(Variant)` truth reflects the new internal model
   - completion evidence:
     - pointer-helper variant tests pass
5. `vmm-e4`
   - kind: `delivery`
   - priority: `P2`
   - depends on: `vmm-e2`
   - title: `Reconcile windows_variant and SAFEARRAY interactions`
   - outcome:
     - COM boundary translation remains correct under the new substrate
   - completion evidence:
     - COM and SAFEARRAY-sensitive tests pass
6. `vmm-e5`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-e3`, `vmm-e4`
   - title: `Run and reconcile Variant old/new correctness, perf, and memory matrix`
   - outcome:
     - the Variant/value migration is validated as a whole
   - completion evidence:
     - relevant matrix rows and discrepancy classifications are updated.
7. `vmm-e6`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-e3`, `vmm-e4`, `vmm-e5`
   - title: `Complete intrinsic internal Variant and SAFEARRAY carrier migration`
   - outcome:
     - the canonical runtime late-bound/general value carrier is exactly
       Windows/COM `VARIANT`, with real `BSTR`, `SAFEARRAY*`, and
       interface-pointer payloads in the union slot
   - current staged progress:
     - stage 1 landed: canonical `Variant` is now back to the exact 16-byte
       carrier for scalar, string, and `ObjectRef`-backed object lanes, with
       ownership/clone/drop driven from payload bytes rather than semantic
       side-owned storage
     - stage 2 landed: canonical `SafeArray` now owns a raw SAFEARRAY-style
       descriptor plus contiguous typed payload storage instead of a public
       semantic `Vec<RuntimeValue>` carrier masquerading as the internal truth
     - stage 3 landed: intrinsic typed SAFEARRAY element-vartype preservation
       now covers `VT_ARRAY | VT_VARIANT`, scalar lanes, `VT_BSTR`,
       `VT_DECIMAL`, `VT_DISPATCH`, and `VT_UNKNOWN`, and the Windows bridge
       plus host lanes assert those typed results directly instead of
       normalizing them away
     - residual scope decision landed: `BindingHandle` is an internal host /
       dynlink token, not a VBA/COM value-model type, so it is explicitly
       narrowed out of the canonical `Variant` carrier scope instead of being
       misclassified as a missing SAFEARRAY/VARIANT migration lane
     - correction landed: the preceding stages are intermediate progress only;
       this bead remains open until the internal late-bound/general value used
       for `Dim x` is exactly the Windows/COM `VARIANT` representation
     - progress landed: `Variant` now supports `VT_ARRAY | VT_VARIANT` and
       stores the array payload as a real owned `SAFEARRAY*` union pointer,
       including clone/drop and runtime bridge coverage; this does not close the
       bead because `RuntimeValue` remains a separate semantic enum carrier
     - progress landed: VM registers now store normal VBA values as
       `RuntimeSlot::Variant(Variant)`, with `BindingHandle` retained only as an
       explicit non-VBA side token; `ReDim` now creates typed SAFEARRAY payloads
       for the declared element type and exposes descriptor bounds needed by
       `ReDim Preserve`; this still does not close the bead because
       `RuntimeValue` remains a semantic projection across helper functions,
       host callback surfaces, SafeArray element APIs, and `ComValue` bridges
     - progress landed: `ForEachIteratorState` and WithEvents binding state now
       retain `RuntimeSlot` values instead of `RuntimeValue` values, reducing
       the remaining semantic-storage footprint while keeping `RuntimeValue` as
       a boundary/projection helper
     - progress landed: JIT `RtSlot` is no longer a private tag/payload
       structure; it is a transparent owner of the canonical 16-byte
       `Variant`, generated Long fast paths now inspect `VT_I4` plus the
       VARIANT union payload, and load/branch helpers preserve overwrite/drop
       and semantic truthiness behavior for non-scalar carriers
     - progress landed: JIT WithEvents retained state now uses
       `JitRuntimeSlot`, keeping normal VBA values in `Variant` form and
       retaining `BindingHandle` only as an explicit non-VBA side token; JIT
       `ReDim` helpers now create typed SAFEARRAY payloads for declared array
       element types
     - progress landed: object-valued canonical runtime `Variant` cells now use
       `VT_UNKNOWN` for the IUnknown-backed object identity/lifetime lane, and
       VM/JIT `VarPtr(Variant)` now exposes the actual internal `Variant` slot
       cell instead of a copied runtime-pointer-helper projection
     - progress landed: COM runtime event callback queues now retain callback
       arguments as `Variant` through `ComEventCallbackValue` instead of
       retaining `ComValue`; existing COM/HAL polling and callback-argument
       APIs still project those queued variants back to `ComValue` at the
       boundary
     - progress landed: dynamic COM request/event payloads now use
       `DynamicValue` as a `Variant`-backed carrier instead of aliasing
       `ComValue`; `DynamicCallArg` and `DynamicEventPayload` therefore retain
       variants internally and project to `ComValue` only at COM conversion
       boundaries
     - progress landed: COM invoke requests now retain argument payloads as
       `ComInvokeValue`, a `Variant`-backed carrier; `ComInvokeArg`
       constructors continue to accept `ComValue` for boundary compatibility,
       but `ComInvokeRequest.args` no longer own semantic `ComValue` payloads
     - progress landed: polled COM callback payloads now retain arguments as
       `ComCallbackValue`, a `Variant`-backed carrier; callback consumers can
       still project those payload values to `ComValue`, but
       `ComCallbackPayload.args` no longer own semantic `ComValue` payloads
     - progress landed: `SafeArray` now exposes Variant-native construction,
       element-read, and element-replacement APIs; the legacy `RuntimeValue`
       element APIs are compatibility projections over the Variant path, and
       clone/equality read canonical Variant elements instead of semantic
       `RuntimeValue` elements
     - progress landed: the Windows `VARIANT`/`SAFEARRAY` bridge now reads and
       writes internal `SafeArray` payloads through `Variant` element vectors;
       typed scalar conversion still performs per-element projection where
       required, but the bridge no longer owns `Vec<RuntimeValue>` as the array
       carrier
     - progress landed: VM and JIT `ReDim` / `ReDim Preserve` helper paths now
       allocate and preserve typed SAFEARRAY elements as canonical `Variant`
       values using `variant_elements()` / `replace_variant_elements()`; the
       compatibility projection bridge now handles `VT_UI1` Byte variants
     - progress landed: VM and JIT intrinsic array literal/append helpers now
       construct and extend internal arrays through `Variant` element vectors
       instead of retaining `RuntimeValue` vectors inside the resulting
       SAFEARRAY payloads
     - progress landed: VM `For Each` array materialization now reads
       SAFEARRAY payloads via `variant_elements()` and retains iterator items
       directly as `RuntimeSlot::Variant`
     - progress landed: VM semantic array get/set now read and replace
       SAFEARRAY payload slots through `variant_elements()` /
       `replace_variant_elements()`, keeping `RuntimeValue` only at the
       semantic API boundary
     - progress landed: Windows `IEnumVARIANT` materialization now builds the
       resulting internal array through `SafeArray::from_variants()` after COM
       projection/binding, rather than retaining a `RuntimeValue` vector
     - progress landed: pointer-helper array projection paths now read
       SAFEARRAY payloads through `variant_elements()` and only project to byte
       buffers or Windows `VARIANT` values at the pointer-helper boundary
     - progress landed: SafeArray debug formatting and pointer byte-array
       readback now use Variant element APIs rather than the legacy
       `RuntimeValue` element projection
     - progress landed: HAL dynamic-link descriptor multi-call now exposes a
       Variant-native path (`invoke_descriptor_variants` /
       `invoke_bound_variants`), and both VM and JIT external descriptor calls
       use exact slot-level Variant reads/writes for return values and ByRef
       writebacks; the older `RuntimeValue` multi-call remains only as a
       compatibility projection for HAL adapters and legacy symbol-token APIs
     - progress landed: the HAL COM/dynlink projection property now asserts
       stable `ObjectRef::compat_identity()` instead of full `ObjectRef`
       pointer equality for two separate `CreateObject` calls, matching the
       IUnknown/ObjectRef identity model where separate object instances can
       share a compatibility identity without being the same pointer identity
     - progress landed: COM event callback argument retrieval now has a
       Variant-native transport from queued native callback state through
       `WindowsComBridge::event_callback_variant()` and
       `ComHal::event_callback_variant()` into VM/JIT slot writes; the older
       `event_callback_arg()` remains as a compatibility projection
     - progress landed: VM project-dynamic `ParamArray` binding now builds the
       internal SAFEARRAY payload from retained argument `Variant` values with
       `SafeArray::from_variants()` instead of projecting each argument to
       `RuntimeValue` before array construction; the dynamic-dispatch binder
       still exposes the existing semantic API boundary, so this is progress
       toward the exact carrier, not closure
     - progress landed: JIT descriptor-backed external calls now route both
       single-argument and multi-argument descriptor calls through
       `invoke_descriptor_variants()` using slot `Variant` reads; the
       no-descriptor legacy symbol-token path remains a compatibility
       projection through `RuntimeValue`
     - progress landed: HAL conformance multidimensional SAFEARRAY shape
       probing now constructs the probe array through `SafeArray::from_variants_nd()`
       with canonical `Variant` elements instead of using the legacy
       `RuntimeValue` compatibility constructor
     - progress landed: VM/JIT `StrPtr`, generic `VarPtr`, `VarPtr(String)`,
       and `ObjPtr` now construct their pointer-helper boundary cells from
       canonical slot `Variant` reads instead of first flattening through
       semantic `RuntimeValue` slots; the generic array-buffer `VarPtr(buf(0))`
       lane remains on the byte-payload helper path, and typed `VT_UI1`
       SAFEARRAY replacement now coerces canonical `Empty` defaults back to
       zero so byte-buffer pointer and native-read lanes remain green
     - progress landed: VM descriptor-backed external calls now match the JIT
       descriptor path by routing both single-argument and multi-argument
       descriptor calls through `invoke_descriptor_variants()` using slot
       `Variant` reads; the no-descriptor legacy symbol-token path remains a
       compatibility projection through `RuntimeValue`
     - progress landed: VM project-dynamic argument binding now carries bound
       target-procedure arguments as `RuntimeSlot` values and writes callee
       slots through a slot-native inline invocation path; the project-symbol
       callback wrapper remains a semantic compatibility boundary
     - progress landed: VM and JIT dynamic COM dispatch result writes now
       preserve `DynamicValue` results as destination-slot `Variant` payloads,
       with legacy COM error-tag normalization performed directly on the
       Variant before the slot write
     - progress landed: JIT WithEvents retained-value get/set/owner-search
       helpers now read, write, and compare normal retained VBA values as
       `Variant` slots directly; `BindingHandle` remains an explicit
       non-VBA control-plane token and explicit `VT_I4` zero remains the
       clear/remove carrier
     - progress landed: VM `For Each` next-item delivery now writes retained
       `RuntimeSlot` values directly into the item slot, and VM WithEvents
       retained-value get/set/owner-search helpers now keep normal retained VBA
       values on `Variant` slots; the project COM subscription sync still
       projects at its object-boundary check only
     - progress landed: VM project-dynamic dispatch now returns project-routed
       member results as `RuntimeSlot` values and writes dispatch destinations
       directly from those slots; object `_NewEnum` fallback also wraps the
       retained `DynamicValue` `Variant` into a slot before semantic array
       inspection
     - support audit landed:
       `docs/evidence/value_model_migration/SAFEARRAY_RUNTIMEVALUE_PROJECTION_AUDIT_2026-04-23.md`
       records that remaining `RuntimeValue` SAFEARRAY API hits are currently
       compatibility API definitions and tests/property fixtures by scan; this
       does not close the public compatibility API classification question
     - progress landed: HAL COM dispatch now exposes Variant-native direct and
       dynamic dispatch methods, and the standard host routes the legacy
       `RuntimeValue` COM dispatch methods as compatibility projections over
       those Variant paths
     - progress landed: VM object `_NewEnum` array inspection now reads
       `Variant::as_safearray()` from the returned slot before materializing
       iterator items, leaving `RuntimeValue` projection only for an
       unsupported-value diagnostic
     - progress landed: JIT execution context result extraction now exposes
       `extract_user_variants()` for exact user-visible `Variant` slot
       carriers; `extract_user_values()` remains as a public compatibility
       projection over that Variant-native result surface
     - progress landed: VM execution context result extraction now exposes
       `snapshot_variants()` for exact user-visible `Variant` slot carriers;
       `snapshot()` and `snapshot_values()` remain public compatibility
       projections over that Variant-native result surface
     - progress landed: VM and JIT public execution snapshot helpers now have
       Variant-native result surfaces (`execute_and_snapshot_variants*()` and
       `cranelift::execute_bytecode_rtslot_variants()`), while existing
       `RuntimeValue` helper APIs remain compatibility projections over those
       Variant result paths where the execution path can produce Variant
       carriers
     - progress landed: host/project execution snapshot APIs now have
       Variant-native companion surfaces for source execution, project
       execution, prepared project session snapshots, and direct session slot
       reads; existing host `RuntimeValue` snapshot APIs remain compatibility
       projections for existing callers
     - progress landed: host bundle execution now has a Variant-native snapshot
       companion (`execute_bundle_with_variant_snapshot()`), leaving the
       existing bundle `RuntimeValue` snapshot API as a compatibility projection
     - progress landed: immediate sessions now expose `snapshot_variants()` over
       the prepared project runtime session, while immediate evaluation display
       remains on the existing `RuntimeValue` projection surface
     - progress landed: host legacy slot snapshots now project directly from
       retained `Variant` snapshot carriers; `ProjectRuntimeSession::snapshot_slots()`
       and the host slot-snapshot test helpers use the Variant companions and
       `Variant::project_compat_slot_i32()` instead of detouring through a
       `Vec<RuntimeValue>` first
     - progress landed: `Variant::project_compat_slot_i32()` now projects the
       exact legacy slot subset directly from retained `Variant` payloads rather
       than converting through `to_runtime_value()` first, preserving the
       existing failure diagnostics for non-representable carriers
     - progress landed: `Variant::try_from_compat_slot_i32()` now decodes the
       legacy tag subset directly into canonical `Variant` carriers instead of
       detouring through `RuntimeValue::from_compat_slot_i32()`
     - progress landed: JIT `JitContext::read_slot()` / `write_slot()` now
       treat the `RuntimeValue` slot API as a compatibility projection over
       `read_variant_slot()` / `write_variant_slot()` instead of maintaining a
       separate non-Variant slot read/write path
     - progress landed: JIT `oxrt_array_literal` and `oxrt_array_append` now
       consume exact slot `Variant` carriers directly and write the resulting
       SAFEARRAY back through `write_variant_slot()`, removing the old
       read-`RuntimeValue`/rebuild-`Variant` detour from that helper family
     - progress landed: pointer-helper string/byte-array payload readback now
       has Variant-native companions, and JIT external-call pointer writeback
       uses those Variant readback surfaces directly instead of converting a
       temporary `RuntimeValue` payload back into `Variant`
     - progress landed: VM semantics now has Variant-native array get/set
       companions, and JIT `oxrt_array_get` / `oxrt_array_set` use those
       companions with Variant slot reads/writes so the array carrier and
       source element no longer detour through `RuntimeValue`
     - progress landed: `ComValue` Variant bridges now convert directly against
       `Variant` accessors and constructors; the `RuntimeValue` bridge methods
       remain compatibility projections rather than the intermediate carrier for
       normal COM value conversions
     - progress landed: embedded host procedure invocation now has
       Variant-native request/result companions over a VM Variant invocation
       path; existing embedded `RuntimeValue` APIs project into and out of that
       Variant path for compatibility callers
     - progress landed: immediate procedure invocation now parses arguments as
       `Variant` and invokes through the host Variant procedure path; its
       display result remains a compatibility projection because the Immediate
       Window exposes formatted values rather than the internal runtime carrier
     - progress landed: debugger frame value projection now reads `Variant`
       slots first and exposes a `DebugFrameVariantValue` companion; existing
       debugger frame/evaluation values remain compatibility projections for
       UI/display callers
     - progress landed: host class member invocation now has a Variant-native
       companion and uses a `Variant` carrier for the implicit `Me` argument;
       existing `RuntimeValue` member invocation remains a compatibility
       projection
     - progress landed: host COM callback ingress and project-handler dispatch
       now have Variant-native companion APIs; `ComEventCallbackVariantDispatch`
       preserves callback payloads as exact `Variant` values and dispatches
       through the VM Variant procedure path, while existing `RuntimeValue`
       callback APIs remain compatibility projections
     - progress landed: dynamic-link legacy symbol-token invocation now has a
       Variant-native companion; VM/JIT no-descriptor external-call sites pass
       and write exact slot `Variant` carriers through
       `DynamicLinkHal::invoke_symbol_variant()`, while `invoke_symbol()`
       remains a compatibility API for existing HAL callers
     - progress landed: HAL dynamic COM bridge invocation now returns
       `DynamicValue` from `ComHal::dispatch_invoke_dynamic_variant()`
       directly, avoiding the older `RuntimeValue` detour for dynamic COM
       invocation payloads
     - remaining blocker: `vmm-e6` still remains open until the interpreter/JIT
       helper seams, public VM/JIT compatibility snapshot/result APIs, HAL
       surfaces that still use semantic values by contract, legacy dynamic-link
       symbol APIs, legacy SafeArray element compatibility APIs, COM
       compatibility projection APIs that still expose `RuntimeValue`,
       embedded/immediate/debugger compatibility display APIs that still expose
       `RuntimeValue`, and remaining manual pointer-helper / registry
       compatibility seams are audited and either migrated to exact BSTR,
       Windows/COM `VARIANT`, and SAFEARRAY carriers or explicitly classified
       as projection boundaries outside the internal value model
   - completion evidence:
     - the workset can describe the internal late-bound/general value carrier
       as exactly Windows/COM `VARIANT`, not only native-shaped or
       boundary-correct
8. `vmm-e7`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-e6`
   - title: `Run Variant/SAFEARRAY intrinsic closure checklist`
   - outcome:
     - the final Variant-family `implemented` / `projected` / `bounded`
       classification is recorded before epic closure
   - completion evidence:
     - the workset and final report inputs explicitly state whether the
       Variant/SAFEARRAY lane is intrinsically migrated or still bounded, with
       the checklist result recorded in
       `docs/evidence/value_model_migration/VARIANT_SAFEARRAY_INTRINSIC_CLOSURE_CHECKLIST_2026-04-23.md`.

Post-`vmm-d6` rollout refresh:

1. `vmm-e1` starts at the current bounded carrier in
   `crates/oxvba-runtime/src/variant.rs`
   - current known limit:
      the exact 16-byte canonical carrier now covers scalar, string, and
      `ObjectRef`-backed object values, and array-intent now routes through the
      intrinsic SAFEARRAY carrier; `BindingHandle` is explicitly excluded from
      canonical Variant scope because it is an internal non-VBA token
   - rollout rule:
     the bead must either remove those subset limits under the new carrier or
     leave behind an explicit retained-adapter decision tied back to the fact
     pack.
2. `vmm-e2` owns the semantic/carrier reconciliation seam across:
   - `crates/oxvba-runtime/src/runtime_value.rs`
   - `crates/oxvba-com/src/model.rs`
   - `crates/oxvba-com/src/windows_variant.rs`
   - current compat-slot and coercion seams that still assume the bounded
     runtime `Variant` subset
   - rollout rule:
     this bead is where the old semantic-vs-compat split must be made honest,
     not left as an accidental half-migration.
3. `vmm-e3` must explicitly carry forward the existing exact pointer-helper
   variant lanes in
   `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`
   - scalar container
   - decimal container
   - object-container support
   - array-container support
   - rollout rule:
      `VarPtr(Variant)` remains a first-class observable seam, not a
      best-effort follow-up.
4. `vmm-e4` must explicitly carry forward the current COM/SAFEARRAY-heavy
   coverage in:
   - `crates/oxvba-com/src/windows_variant.rs`
   - `crates/oxvba-host/tests/com_client_end_to_end.rs`
   - rollout focus:
     decimal, typed SAFEARRAY, `VT_UNKNOWN`, `VT_DISPATCH`, and exact
     boundary rebind behavior
   - rollout rule:
     these tests are part of the migration contract, not optional hardening.
5. `vmm-e5` follows the validation discipline established by `vmm-d6`
   - run a paired boundary bundle
   - run the relevant conformance/spec-backed subset
   - run paired perf and memory artifacts
   - classify old/new-independent failures separately from migration-induced
     regressions
   - prefer a completed bounded paired perf artifact over a longer rerun that
     stalls or leaves only partial materialization.

### 11.9 Epic F Bead Set: Interface Identity and COM Event Transport

Parent:

1. `vmm-f`
   - close condition:
     - interface identity truth is reconciled
     - event payload/object identity truth is reconciled
     - retained wrapper decisions are explicit.

Current repo grounding after `vmm-e5`:

1. Canonical runtime/interface identity is semantic, not raw COM-pointer
   storage, but it is no longer token-only.
   - `RuntimeValue` now carries `ObjectRef` as the canonical object lane and
     `BindingHandle` for non-object binding identity.
   - canonical object-valued `Variant` state now also carries `ObjectRef`.
2. Native COM pointer truth and identity dedup are currently owned by the
   Windows COM runtime state.
   - `crates/oxvba-com/src/windows_runtime_state.rs` is the current source of
     truth for retained native dispatch pointers, native `IUnknown` anchors,
     native-result dedup, subscription state, and callback queues.
   - `vmm-f2` has now replaced the token-only runtime object lane with
     canonical `ObjectRef`, tightened the retained/native lane into an explicit
     retained `IUnknown` identity / lifetime anchor, and aligned the runtime
     object base with `IUnknown` lifetime semantics rather than leaving
     identity keyed only by retained `IDispatch*`.
3. The current observable `VT_DISPATCH` / `VT_UNKNOWN` contract is already
   explicit enough to drive the migration.
   - object-capable `VT_DISPATCH` results now rebind to `ObjectRef` and remain
     invokable through the COM/runtime seams
   - `VT_UNKNOWN` currently means:
     rebind through `IDispatch` if `QueryInterface(IDispatch)` succeeds;
     otherwise fail deterministically on the bounded nondispatch diagnostic
     path.
4. COM event transport is currently split into two materially different lanes.
   - native connection-point callbacks now enter through `ComValue` at the COM
     boundary, but the retained `ComRuntimeState` callback queue stores
     arguments as `Variant` via `ComEventCallbackValue`
   - projected event triggers now use `ComInvokeArg` / `ComValue` at the
     boundary and project into retained `Variant` queue payloads rather than
     retaining semantic `ComValue` in the callback queue
   - dynamic COM request/event payloads now retain `DynamicValue` as a
     `Variant`-backed carrier instead of aliasing `ComValue`
   - COM invoke request arguments now retain `ComInvokeValue` as a
     `Variant`-backed carrier instead of owning `ComValue` directly
   - polled callback payload arguments now retain `ComCallbackValue` as a
     `Variant`-backed carrier instead of owning `ComValue` directly
   - source-interface callbacks remain explicitly bounded to the existing
     narrow path (`single i32` source-interface sink support; broader COM-EVT-B
     still unsupported in the current lane)
5. The current defining correctness rows for this epic are already present and
   should remain the rollout baseline.
   - `crates/oxvba-host/tests/com_client_end_to_end.rs`
     - `object_variant_results`
     - `plain_unknown`
   - `crates/oxvba-host/tests/com_early_project_end_to_end.rs`
     - `registered_testeventserver_withevents_callback_*`
   - these rows were re-run after `vmm-e5` and remained green, so the F-lane
     starts from a stable post-Variant baseline rather than a speculative
     branch point.

Child beads:

1. `vmm-f0`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-e5`, `vmm-b4`, `vmm-c7`
   - title: `Roll out interface identity and COM event transport child beads`
   - outcome:
     - create the executable child bead set for interface/event work
   - completion evidence:
     - interface, event, and validation child beads exist explicitly
2. `vmm-f1`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-f0`, `vmm-b3`
   - title: `Publish interface identity target and retained-wrapper decision set`
   - outcome:
     - the intended internal identity model is fixed before broad edits
   - completion evidence:
     - interface-identity decisions are documented against evidence
     - decision note:
       `docs/evidence/value_model_migration/INTERFACE_IDENTITY_AND_RETAINED_WRAPPER_DECISIONS_2026-04-21.md`
3. `vmm-f2`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-f1`
   - title: `Reconcile internal interface identity carrier with COM pointer truth`
   - outcome:
     - token-only runtime object identity is replaced with canonical `ObjectRef`
     - runtime object/base structure becomes `IUnknown`-implementing
     - runtime object lifetime is driven through `AddRef` / `Release`
     - retained wrapper becomes explicitly `IUnknown`-anchored without keeping
       `ObjectHandle` as a separate canonical identity token
   - completion evidence:
     - identity-sensitive runtime/COM tests pass
     - object lifetime/refcount ownership is documented honestly
     - retained wrapper identity/lifetime ownership is documented honestly
     - `ObjectHandle` is removed from the live runtime crates and support
       harnesses, with `ObjectRef` carried end-to-end instead
   - landed 2026-04-22:
     - runtime, VM, JIT, HAL, host, compiler metadata, and support harnesses
       now carry `ObjectRef` instead of canonical `ObjectHandle`
     - `crates/oxvba-runtime/src/object_ref.rs` now owns the canonical runtime
       `IUnknown`-style identity/refcount substrate
     - paired memory smoke
       `value_model_memory_vmf2-mem-identity-smoke` records the live
       identity-carrier delta as `ObjectIdentityCarrier`
4. `vmm-f3`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-f2`
   - title: `Repair VT_UNKNOWN and VT_DISPATCH lanes under the new model`
   - outcome:
     - object transport under the new model is correct
   - completion evidence:
     - `VT_UNKNOWN` / `VT_DISPATCH` tests and imported-reference tests pass
   - landed 2026-04-22:
     - retained COM result rebinding now returns the retained
       `ComBinding.runtime_object` instead of rebuilding a fresh `ObjectRef`
       from the compat id
     - repeated late-bound and imported-reference object results now preserve
       identical retained `ObjectRef` identity across repeated
       `VT_DISPATCH` / dispatch-capable `VT_UNKNOWN` rebinds
     - bounded nondispatch `VT_UNKNOWN` scalar and array diagnostics remain
       unchanged
     - evidence note:
       `docs/evidence/value_model_migration/VT_DISPATCH_VT_UNKNOWN_OBJECT_IDENTITY_2026-04-22.md`
5. `vmm-f4`
   - kind: `support`
   - priority: `P2`
   - depends on: `vmm-f2`, `vmm-f3`
   - title: `Roll out event-transport child beads after interface identity landing`
   - outcome:
     - refresh the exact child set for event payload/callback work if one more
       bounded discovery pass is needed
   - completion evidence:
     - event-transport lane has an honest next delivery path
   - landed 2026-04-22:
     - one bounded discovery pass was still needed after the `ObjectRef` /
       retained-identity landing, but it did not require further child-bead
       expansion
     - the exact `vmm-f5` delivery scope is now explicit:
       native callback payloads already store `Vec<ComValue>`, but
       `ComRuntimeState::take_polled_callback()` still rebuilds callback-object
       identity from a compat token instead of returning the retained
       `ObjectRef`
     - projection-triggered event callbacks still require legacy `i32`
       argument transport in
       `queue_projection_event_callbacks_shared(...)` before being widened back
       into `ComValue`
     - callback/subscription tokens remain intentional control-plane tokens;
       the delivery work is object identity and payload transport, not removing
       callback/subscription token handles
6. `vmm-f5`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-f4`
   - title: `Reconcile COM event payload storage and callback identity`
   - outcome:
     - event transport works correctly under the new representation
   - completion evidence:
     - event-related tests pass and discretionary no-change decisions are
       documented where applicable
   - concrete delivery scope after `vmm-f4`:
     - preserve retained `ObjectRef` identity through queued callback payloads
       instead of reconstructing callback objects from compat ids
     - widen projection-triggered event callback argument transport onto
       `ComValue` / `RuntimeValue` carrier paths instead of requiring legacy
       `i32` callback-argument arrays
     - keep callback/subscription tokens only as queue-control identifiers
       unless new evidence shows VBA-visible identity pressure on those tokens
   - landed 2026-04-22:
     - `ComEventSubscription` / `ComEventCallback` now retain `ObjectRef`
       directly, and `ComCallbackPayload.object` returns the retained callback
       object instead of rebuilding from a compat id
     - projection-triggered event callbacks now derive payloads from
       `ComInvokeArg` / `ComValue` rather than requiring legacy `i32`
       callback-argument transport before widening back into semantic values
     - the host callback surface now preserves callback object and event
       metadata in `ComEventCallbackDispatch`
     - callback/subscription tokens remain explicit control-plane identifiers;
       this bead did not reopen that no-change decision
     - evidence note:
       `docs/evidence/value_model_migration/COM_EVENT_CALLBACK_IDENTITY_AND_PAYLOAD_2026-04-22.md`
7. `vmm-f6`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-f3`, `vmm-f5`
   - title: `Run and reconcile interface and event old/new matrix`
   - outcome:
     - interface/event migration results are validated and classified
   - completion evidence:
     - interface/event matrix rows are updated.
   - landed 2026-04-22:
     - paired run `vmf6-interface-event-matrix-r3` passes on both baseline and
       candidate for `dispatch_boundary`, `events_identity`,
       `dispatch_exception_details`, `dispatch_exception_resume_next`,
       `dispatch_exception_rich_excepinfo`, `event_callback_handler_body`, and
       `event_callback_value_payload`
     - the candidate-side matrix closure required fixing two stale host harness
       assumptions: broad VM/JIT snapshot suites in
       `com_client_end_to_end.rs` and `com_early_project_end_to_end.rs` were
       still comparing retained `ObjectRef` allocation identity rather than
       observable object identity
     - the matrix closes with no remaining bounded interface/event divergence
       between the fixed baseline and migrated head
     - evidence note:
       `docs/evidence/value_model_migration/INTERFACE_AND_EVENT_MATRIX_2026-04-22.md`

### 11.10 Epic G Bead Set: Struct / UDT / Native ABI / Pointer-Helper

Parent:

1. `vmm-g`
   - close condition:
     - ABI-sensitive lanes are reconciled
     - unsupported cases are explicit
     - layout-sensitive rows are complete
     - the intrinsic struct/UDT/layout closure checklist bead is complete.

Child beads:

1. `vmm-g0`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-e5`, `vmm-b4`, `vmm-c7`
   - title: `Roll out struct, UDT, native ABI, and pointer-helper child beads`
   - outcome:
     - create the executable child bead set for ABI/layout work
   - completion evidence:
     - pointer-helper, native declare, UDT/layout, and validation beads exist
   - landed 2026-04-22:
     - post-`ObjectRef` audit confirms the child set remains honest; no extra G
       children are required before delivery resumes
     - `vmm-g1` is now explicitly scoped to
       `crates/oxvba-runtime/src/pointer_helpers.rs` plus
       `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`, with the
       surviving bounded gaps called out directly:
       `VarPtr(Variant)` object and array containers still reject explicitly,
       and `ObjPtr` stability now depends on the migrated `ObjectRef`
       substrate rather than compat-handle projection
     - `vmm-g2` is explicitly scoped to native declare/writeback seams in
       `windows_ffi_bridge`, host native declare tests, runtime helper
       writeback application, and HAL writeback hooks, rather than the already
       reconciled COM event/object lanes
     - `vmm-g3` remains conditional and should only reopen the child structure
       if `vmm-g2` exposes additional UDT/layout-sensitive work beyond the
       current bounded UDT subset and native ABI ambiguity register
     - the next ready delivery bead is `vmm-g1`
2. `vmm-g1`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-g0`
   - title: `Reconcile pointer-helper ABI-sensitive cells under the new model`
   - outcome:
     - pointer-exposed cells line up with the migrated substrate
   - completion evidence:
     - pointer-helper tests pass and cell truth is documented
   - landed 2026-04-22:
     - `ObjPtr(obj)` now returns the raw runtime `IUnknown` identity pointer
       for live `ObjectRef` values instead of a synthesized compat-identity
       token
     - runtime object pointer cells now store the `IUnknown` pointer value when
       variable-storage style indirection is required
     - `VarPtr(v As Variant)` now supports object-valued variants via
       `VT_UNKNOWN` + retained `IUnknown*`
     - `VarPtr(v As Variant)` now supports array-valued variants via
       `VT_ARRAY | VT_VARIANT` + real `SAFEARRAY`
     - bounded verification stayed green for runtime unit tests, host pointer
       helper E2E lanes, and native declare string/writeback lanes
     - evidence note:
       `docs/evidence/value_model_migration/POINTER_HELPER_ABI_CELL_RECONCILIATION_2026-04-22.md`
3. `vmm-g2`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-g1`
   - title: `Reconcile native declare and writeback lanes`
   - outcome:
     - declare/native writeback behavior remains correct
   - completion evidence:
     - native declare tests pass under the new representation
   - landed 2026-04-22:
     - no additional runtime adapter rewrite was required in this bead; the
       existing standard dynlink/writeback path stayed correct under the
       migrated substrate
     - executable Windows host-backed coverage now explicitly proves the
       supported `ByRef` scalar subset for `LongPtr`, `Boolean`, `Integer`,
       `Single`, `Double`, `LongLong`, `Currency`, and `Date`
     - existing pointer-driven writeback lanes for `StrPtr(...)`,
       `VarPtr(s As String)`, and `VarPtr(buf(0))` remained green alongside the
       widened scalar subset
     - evidence note:
       `docs/evidence/value_model_migration/NATIVE_DECLARE_WRITEBACK_RECONCILIATION_2026-04-22.md`
4. `vmm-g3`
   - kind: `support`
   - priority: `P2`
   - depends on: `vmm-g2`, `vmm-b3`
   - title: `Roll out UDT and layout-sensitive child beads after native ABI reconciliation`
   - outcome:
     - refresh the exact child set for any remaining UDT/layout work if one more
       bounded discovery pass is required
   - completion evidence:
     - UDT/layout lane has an honest next delivery path
   - landed 2026-04-22:
     - the post-`vmm-g2` audit did not expose a missing child bead
     - the remaining open delivery scope still fits the existing `vmm-g4`
       truth/docs pass and `vmm-g5` matrix pass
     - current bounded UDT/runtime truth remains the non-boundary deterministic
       subset already implemented in compiler/host execution, while broader
       native ABI and layout-sensitive closure remains explicitly bounded by the
       existing HAL ambiguity/spec register
     - the next ready delivery bead remains `vmm-g4`
5. `vmm-g4`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-g3`
   - title: `Reconcile UDT/layout-sensitive truth and docs`
   - outcome:
     - remaining UDT/layout-sensitive behavior is implemented or bounded
   - completion evidence:
     - layout-sensitive tests/docs are updated and unsupported cases are explicit
   - landed 2026-04-22:
     - [UDT_LAYOUT_BOUNDARY_STATUS_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/UDT_LAYOUT_BOUNDARY_STATUS_2026-04-22.md)
       now records the honest migrated truth for the UDT lane
     - compiler/runtime truth remains the bounded non-boundary deterministic
       subset: declaration tolerance, flattened field access, nested-field
       expansion, same-type whole-value copy lowering, and cross-type rejection
     - focused host regression lanes for whole-UDT copy, nested UDT access, and
       cross-type rejection passed on the migrated runtime
     - public `udt_*` conformance fixtures passed in both VM and JIT
     - broad native-layout and UDT-byref boundary parity remains explicitly
       deferred to the ABI contract and ambiguity register rather than being
       claimed as closed by the value-model migration
6. `vmm-g5`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-g1`, `vmm-g2`, `vmm-g4`
   - title: `Run and reconcile ABI and layout old/new matrix`
   - outcome:
     - ABI/layout migration results are validated and classified
   - completion evidence:
     - ABI/layout matrix rows are updated.
   - landed 2026-04-22:
     - [ABI_LAYOUT_MATRIX_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/ABI_LAYOUT_MATRIX_2026-04-22.md)
       records the closed ABI/layout matrix for the migration
     - paired correctness run `vmg5-abi-layout-r3` passed on both baseline and
       candidate for the selected UDT, pointer-helper, variant-container, and
       native-writeback rows
     - the authoritative post-`ObjectRef` layout delta table remains
       `vmf2-mem-identity-smoke/comparison/layout_metrics.csv`, which captures
       the intentional representation changes in `Variant`,
       `ObjectIdentityCarrier`, and `ComCallbackPayload`
     - remaining old/new ABI/layout differences are now classified as intended
       migration deltas rather than unresolved regressions:
       `IUnknown`-backed object pointer truth and `VarPtr(Variant)` support for
       object/array payload materialization
     - broad native struct-overlay parity and unconstrained UDT-byref native
       ABI parity remain explicitly bounded outside the closed matrix
7. `vmm-g6`
   - kind: `delivery`
   - priority: `P1`
   - depends on: `vmm-g5`
   - title: `Resolve intrinsic struct/UDT layout closure status`
   - outcome:
     - either the scoped internal struct/UDT/native-layout truth is actually
       delivered, or the workset is explicitly narrowed with the user's
       approval before closure is attempted
   - completion evidence:
     - the family truth no longer depends on a silent substitution of bounded
       subset behavior for broad layout closure
8. `vmm-g7`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-g6`
   - title: `Run struct/UDT/layout intrinsic closure checklist`
   - outcome:
     - the final struct-family `implemented` / `projected` / `bounded`
       classification is recorded before epic closure
   - completion evidence:
     - the workset and final report inputs explicitly state whether the
       struct/UDT/layout lane is intrinsically migrated or still bounded.

### 11.11 Epic H Bead Set: Final Matrix, Docs, and Report

Parent:

1. `vmm-h`
   - close condition:
     - final matrix is complete
     - docs reflect the new truth
     - the final report is published and linked from the workset
     - the family closure checklist beads have all passed.

Child beads:

1. `vmm-h0`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-d6`, `vmm-e5`, `vmm-f6`, `vmm-g5`
   - title: `Roll out final matrix, docs, and report child beads`
   - outcome:
     - create the executable child bead set for closure/report work
   - completion evidence:
     - final doc/report closure has an explicit ready path
   - landed 2026-04-22:
     - the existing `vmm-h1` through `vmm-h4` child set remains sufficient
       after the ABI/layout matrix closed
     - no extra closure child bead is required
     - the next ready path is now explicit: refresh truth-surface docs, publish
       the canonical paired artifact index, finalize the discretionary decision
       and mitigation register, then publish the final migration report
2. `vmm-h1`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-h0`
   - title: `Refresh architecture and runtime truth-surface docs after migration`
   - outcome:
     - canonical docs match the migrated truth
   - completion evidence:
     - architecture/runtime/interop docs are updated and linked
   - landed 2026-04-22:
     - [ARCHITECTURE.md](/C:/Work/DnaCalc/OxVba/docs/ARCHITECTURE.md) now
       reflects `BStr`, `ObjectRef`, and the owned semantic `Variant` carrier
       rather than the pre-migration token-era descriptions
     - [VM_ARCHITECTURE.md](/C:/Work/DnaCalc/OxVba/docs/VM_ARCHITECTURE.md)
       now states the current `RuntimeValue` carrier truth and compat-slot
       boundary caveat explicitly
     - [OXVBA_POINTER_HELPERS_CONTRACT_V1.md](/C:/Work/DnaCalc/OxVba/docs/spec/OXVBA_POINTER_HELPERS_CONTRACT_V1.md)
       now matches the current pointer-helper boundary truth for object- and
       array-valued `Variant` materialization and `IUnknown`-backed `ObjPtr`
3. `vmm-h2`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-h0`
   - title: `Publish paired old/new result index for correctness, perf, and memory`
   - outcome:
     - all major artifacts are indexed together
   - completion evidence:
     - correctness/perf/memory artifact sets are linked from one canonical index
   - landed 2026-04-22:
     - [PAIRED_RESULT_INDEX_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/PAIRED_RESULT_INDEX_2026-04-22.md)
       now groups the selected correctness, performance, and memory artifacts by
       migration area
     - [REPORT_INPUT_INDEX.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/REPORT_INPUT_INDEX.md)
       now links that grouped paired-result index alongside the flat artifact
       map
4. `vmm-h3`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-h0`
   - title: `Finalize discretionary decision register and mitigation backlog`
   - outcome:
     - every retained discretionary choice and follow-on mitigation is recorded
   - completion evidence:
     - the decision register and mitigation list are complete and linked
   - landed 2026-04-22:
     - [04_DISCRETIONARY_DECISIONS.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/04_DISCRETIONARY_DECISIONS.md)
       now records the migrated architectural and artifact-selection decisions
     - [06_FURTHER_MITIGATIONS.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/report_inputs/06_FURTHER_MITIGATIONS.md)
       now holds the post-correctness optimization and bounded-follow-on backlog
5. `vmm-h4`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-h1`, `vmm-h2`, `vmm-h3`
   - title: `Publish final value-model migration report`
   - outcome:
     - the migration report is published as the final closure artifact
   - completion evidence:
     - the report sections from section 12 are present
     - the workset links to the final report.
   - landed 2026-04-22:
     - [VALUE_MODEL_MIGRATION_FINAL_REPORT_2026-04-22.md](/C:/Work/DnaCalc/OxVba/docs/evidence/value_model_migration/VALUE_MODEL_MIGRATION_FINAL_REPORT_2026-04-22.md)
       is now the published closure artifact for the migration
     - report inputs `01`, `02`, `03`, and `05` are finalized and aligned with
       the selected paired artifact set
     - the final report closes with the migrated value model as the active
       implementation, a green selected correctness matrix, accepted
       representation-growth deltas recorded explicitly, and the remaining
       non-migration follow-on mitigations called out as backlog rather than
       closure blockers
6. `vmm-h5`
   - kind: `support`
   - priority: `P1`
   - depends on: `vmm-d8`, `vmm-e7`, `vmm-f6`, `vmm-g7`, `vmm-h1`, `vmm-h2`,
     `vmm-h3`, `vmm-h4`
   - title: `Run final migration closure checklist and reissue final report status`
   - outcome:
     - final closure is approved only after the family intrinsic checklist beads
       have passed and the report status matches that result
   - completion evidence:
     - the final report and workset both return to `final` / `complete` only
       after the family closure checklist beads are closed.

### 11.12 Bead-Plan Coverage Check

This second-pass bead plan is only valid if every accepted lane in the workset
is owned by at least one delivery path.

Coverage check:

1. baseline truth and cleanup
   - owned by `vmm-a*`
2. Windows VBA 7.1 x64 fact pack
   - owned by `vmm-b*`
3. old/new matrix, perf, memory, and evidence skeleton
   - owned by `vmm-c*`
4. string/BSTR substrate migration
   - owned by `vmm-d*`
5. Variant/value substrate migration
   - owned by `vmm-e*`
6. interface identity and COM event transport
   - owned by `vmm-f*`
7. struct / UDT / native ABI / pointer-helper reconciliation
   - owned by `vmm-g*`
8. final documentation, result index, and migration report
   - owned by `vmm-h*`.

Readiness check:

1. the first ready bead is `vmm-a0`
2. every epic has a rollout bead
3. every capability epic has one or more delivery beads after the rollout bead
4. the critical path runs through `vmm-a* -> vmm-b* -> vmm-c* -> vmm-d* -> vmm-e*`
   before splitting into `vmm-f*` and `vmm-g*`
5. the closure path runs through `vmm-h*`
6. no representation epic closes without its intrinsic closure checklist bead
7. the final report cannot return to final closure state until `vmm-h5` passes
8. the two deliberate conditional-rollout points are:
   - `vmm-f4` for event transport after interface identity work
   - `vmm-g3` for UDT/layout-sensitive closure after native ABI work.

## 12. Required Final Report

The migration is not complete until a final report exists.

That final report may not be marked `final` while any family intrinsic closure
checklist bead remains open.

The report must include these sections.

### 12.1 Executive result

State clearly:

1. whether the new representation is now the active implementation
2. whether correctness is confirmed against the required matrix
3. whether any known divergences remain
4. whether any rollout gating issues remain.

### 12.2 Representation summary

Describe what changed:

1. string representation
2. Variant/value representation
3. interface identity representation
4. event payload representation
5. pointer-helper and ABI-sensitive representation.

### 12.3 Correctness result

Confirm that everything works after the migration:

1. summarize matrix totals
2. link the old/new correctness artifacts
3. call out any old-implementation bugs that were exposed
4. classify any remaining open issues against the authority hierarchy.

### 12.4 Discretionary decisions

Record every discretionary decision that may later be revisited, for example:

1. places where Windows VBA 7.1 x64 internals could not be fully observed
2. places where OxVba retained a portability wrapper around the Windows model
3. places where a direct internal match was judged dishonest or impractical
4. places where interface/event/layout truth stayed wrapped rather than becoming
   a raw carrier.

Each discretionary decision must record:

1. what was decided
2. why
3. evidence basis
4. revisit trigger.

### 12.5 Performance and memory result

Show:

1. old/new timing results
2. old/new memory results
3. string-workload detail for:
   - small strings
   - long strings
   - many strings
   - code strings
4. COM/native boundary timing where relevant
5. any regressions and any improvements
6. any observed reduction in synthetic helper allocations or boundary copies.

### 12.6 Further mitigations

Describe any follow-up mitigation opportunities, for example:

1. allocation reductions
2. copy elision
3. small-string handling
4. object/interface identity caching
5. event payload transport tightening
6. layout-aligned optimization that does not change semantics.

These are optimization follow-ups only after correctness is already
established.

## 13. Canonical Docs, Code, Tests, and Evidence Affected

This workset is expected to touch, at minimum, the following truth surfaces.

### Docs

1. this workset file
2. `docs/ARCHITECTURE.md`
3. runtime/interop spec notes affected by the migration
4. validation/evidence index files for migrated lanes
5. fact-pack artifacts under `docs/evidence/`
6. final migration report under `docs/evidence/`.

### Runtime / VM / JIT / HAL / COM code

1. `crates/oxvba-runtime/src/bstr.rs`
2. `crates/oxvba-runtime/src/runtime_value.rs`
3. `crates/oxvba-runtime/src/variant.rs`
4. `crates/oxvba-runtime/src/pointer_helpers.rs`
5. `crates/oxvba-runtime/src/coerce.rs`
6. `crates/oxvba-vm/src/semantics.rs`
7. `crates/oxvba-jit/src/slot_abi.rs`
8. `crates/oxvba-jit/src/runtime_helpers.rs`
9. `crates/oxvba-com/src/model.rs`
10. `crates/oxvba-com/src/windows_variant.rs`
11. `crates/oxvba-com/src/windows_runtime_state.rs`
12. `crates/oxvba-com/src/windows_connection_point.rs`
13. `crates/oxvba-com/src/windows_invoke.rs`
14. `crates/oxvba-com/src/dynamic_object.rs`
15. `crates/oxvba-hal/src/conformance.rs`
16. relevant HAL adapter files.

### Tests

1. `crates/oxvba-host/tests/native_declare_string_marshalling_end_to_end.rs`
2. `crates/oxvba-host/tests/pointer_helpers_end_to_end.rs`
3. `crates/oxvba-host/tests/com_client_end_to_end.rs`
4. `crates/oxvba-host/tests/com_client_registered_lane.rs`
5. `crates/oxvba-host/tests/com_early_project_end_to_end.rs`
6. `crates/oxvba-host/tests/imported_collection_newenum_regression.rs`
7. event-related host tests
8. any VM/JIT parity tests that assert runtime value truth
9. any string-heavy conformance or host-backed tests
10. new old/new perf and memory workloads.

### Scripts and evidence

1. existing benchmark scripts:
   - `scripts/run-bench.ps1`
   - `scripts/run-com-early-perf.ps1`
2. new old/new correctness matrix scripts:
   - `scripts/run-value-model-correctness.ps1`
   - `scripts/compare-value-model-results.ps1`
3. new old/new perf scripts:
   - `scripts/run-value-model-string-perf.ps1`
4. new old/new memory scripts:
   - `scripts/run-value-model-memory.ps1`
5. migration evidence roots under `docs/evidence/value_model_migration/`
6. final report artifacts.

## 14. Terminal Condition

This workset is complete only when all of the following are true:

1. the internal data representation migration is implemented
2. the affected docs/code/tests are updated
3. the full old/new matrix has been run against baseline tag and migrated head
4. correctness is confirmed against the authority hierarchy
5. memory and timing results are captured
6. discretionary decisions are explicitly documented
7. the final migration report is published.
8. the family intrinsic closure checklist beads are complete:
   - `vmm-d8`
   - `vmm-e7`
   - `vmm-g7`
   - `vmm-h5`.
