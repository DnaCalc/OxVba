# CORE-1 Host/JIT Expectations and Structured Diagnostics

Date: 2026-07-11
Bead: `bd-59co.2.2.7`
Base: `dd413a1b1adbc4a5baf55a0400eccbdc99eb0a2d`
Matrix route: `CORE-READINESS/CORE-BASELINE-HOST-JIT-DIAGNOSTICS`
Clauses: `COMP-DIAG-001|HOST-HAL-001|HAL-ERR-001|JIT-PARITY-001|WIN-META-001|CONF-DIFF-001`

## Result

The stale host expectation now treats explicit `New Collection` as a supported
JIT shape and proves that the captured local contains an object. The test name
contains `new_collection`, so the bead's named acceptance command executes the
assertion rather than reporting an empty filter.

Windows native-Declare and string/pointer-marshalling tests retain their real VM3
coverage. Their JIT legs still decline before native execution and never fall
back to VM3, but now assert the stable structured diagnostic instead of obsolete
message prose: host phase `Runtime`, code `RUN-E-JIT-UNSUPPORTED`, diagnostic
phase `runtime`, severity `error`, and no VBA error number.

Dynamic COM name-resolution failures now use one constructor on Windows and
non-Windows. It uses `HalError::adapter_fault`, preserving the public HAL
taxonomy as structured code `HAL-E-ADAPTER-FAULT`, with `AdapterFault`, active profile,
`ComActivationDispatch`, `dispatch_invoke`, and no host error number as separate
fields. `COM-E-DYNAMIC-NAME-UNRESOLVED` remains the semantic label in the exact
VM-visible message:

```text
COM-E-DYNAMIC-NAME-UNRESOLVED: dynamic member name `Visible` requires authoritative metadata resolution before COM lowering
```

The COM semantic label remains in `Err.Description`; it is not the structured HAL
code. The current VM fault transport carries the description but not the HAL
diagnostic DTO. Centralizing the constructor removes the platform-only wording
split, preserves `HAL-ERR-001`, and preserves the checked-in cross-platform golden
bytes for this row; the snapshot was not changed for the `Visible` repair.

The repaired golden then exposed a stale `WithEvents` row. Updating that one row
was authorized after its authority chain was confirmed:

- `0a17ae3b` intentionally rejects local `Dim WithEvents` declarations;
- `withevents_local_declaration_is_bind_error` and
  `scanner_rejects_withevents_in_local_scope` enforce the current structured
  symbol diagnostic;
- `docs/evidence/language/MS_VBAL_MODULE_PROJECT_REQUIREMENTS.csv`, row
  `MODPROJ-022`, records that procedural declarations cannot include
  `WithEvents`;
- the full conformance oracle captures record
  `project_model_withevents_requires_class_graph.bas` as an error.

Only that exact golden row changed, from opaque success to the current bind
error. No broad or platform-specific blessing was performed. The final golden
gate remains red on a later, unrelated parse-offset drift described under
Residuals, so this evidence does not claim cross-platform baseline completion.

## Root causes and repairs

1. The host unit test predated JIT Collection support and still called a
   successful execution an expected error. It is now a positive object-result
   assertion.
2. Native-Declare tests matched mutable English text left over from the earlier
   all-JIT-unavailable state. They now inspect the public structured diagnostic
   fields for the current per-shape JIT decline.
3. Windows and non-Windows dynamic-name lowering duplicated error construction
   and had diverged semantically. One shared `HalError::adapter_fault` constructor
   now owns the HAL taxonomy fields and the exact COM-labeled VM-visible
   description for both branches and for the Windows ByRef/writeback route.
4. The `WithEvents` golden row predated the accepted local-declaration legality
   check. The exact row now records the current bind error backed by the symbol,
   binder, MS-VBAL, and Excel-oracle evidence above.

## Observable

| axis | observation |
|---|---|
| result | JIT `New Collection` runs and returns a snapshot containing the Collection object. Native Declare remains VM3-supported and JIT-declined. Dynamic `Visible` resolution still raises through structured HAL code `HAL-E-ADAPTER-FAULT` with the exact COM-labeled message; only diagnostic construction was unified. |
| Full Err | `New Collection` succeeds and raises no runtime fault. A JIT native-Declare decline is a host `PhaseDiagnostic`, not a VBA `Err` mutation: code `RUN-E-JIT-UNSUPPORTED`, runtime/error, `vba_error_number=None`. The dynamic COM golden row remains VBA error `5`, `source="VBAProject"`, the shared `COM-E-DYNAMIC-NAME-UNRESOLVED` description above, and `last_dll_error=0`; the underlying `HAL-E-ADAPTER-FAULT` code is structured at the HAL boundary and is not a VBA Err field. The compile-time `WithEvents` row retains default runtime Err state (`number=0`, empty source/description, `last_dll_error=0`). |
| side effects | The Collection probe creates only the local runtime object. JIT native-Declare decline happens before DLL invocation. All 14 Windows native tests and all 19 Windows string/pointer tests still ran their bounded VM3 native paths and cleanup. The COM repair changes error construction after metadata lookup fails; it does not add dispatch or activation. |
| lifecycle/order | Collection creation precedes snapshot capture and the captured Variant owns the object until snapshot drop. Native JIT rejection occurs during target acceptance before generated execution or VM fallback. VM3 native neighboring tests retain their allocate/call/writeback/free ordering. Dynamic-name resolution tries the platform metadata route, then constructs the shared failure before COM lowering. |
| transport | Collection uses the JIT/runtime Collection route. Native VM3 tests use `DynamicLinkHal`; JIT returns a structured host diagnostic with no VM3 transport. Dynamic COM starts from the same `DynamicCallRequest`; Windows and non-Windows resolution branches converge on `HalError::adapter_fault` (`HAL-E-ADAPTER-FAULT`), and `Fault::from_hal` projects the exact COM-labeled description to VBA Err. |
| balance | This bead added no balance instrumentation and makes no new carrier-balance claim. The diagnostic helper allocates only its owned message. The Collection snapshot object is released normally on drop. The accepted policy-error BSTR repair at the base allowed the golden run to advance past its former balance failure, but the later parse-offset drift prevents a full golden/balance completion claim here. |

## Checks

Environment: Microsoft Windows 11 Pro `10.0.26200`, x64; Rust/Cargo
`1.94.1`.

```text
cargo test -p oxvba-host new_collection
PASS: 1 named test; JIT New Collection succeeded and the snapshot contained an object.

cargo test -p oxvba-host --test native_declare_lane
PASS: 14/14 Windows tests; no skip. Real VM3 native/ABI coverage remained active and the JIT leg asserted structured decline fields.

cargo test -p oxvba-host --test native_declare_string_marshalling_end_to_end
PASS: 19/19 Windows tests; no skip. Real native string/pointer/writeback coverage remained active and every JIT leg asserted structured decline fields.

cargo test -p oxvba-hal dynamic_member_name_unresolved_diagnostic_is_stable -- --nocapture
PASS: 1 named test; `HAL-E-ADAPTER-FAULT`, kind, profile, capability, operation, host-error field, exact `COM-E-DYNAMIC-NAME-UNRESOLVED` message, and diagnostic metadata.

cargo test -p oxvba-hal
PASS: 159/159 unit tests plus HAL binary/doc-test targets; includes Windows dynamic-name, native COM, typelib, event, and conformance neighbors.

cargo check -p oxvba-hal --lib --target x86_64-unknown-linux-gnu
PASS: the non-Windows shared diagnostic branch compiles. Existing target-specific unused/dead-code warnings remain in runtime/COM/HAL neighbors; this compile-only check is not the pinned Linux baseline.

cargo test -p oxvba-bind --test bind_roundtrip withevents_local_declaration_is_bind_error -- --exact
PASS: 1 named binder authority test.

cargo test -p oxvba-symbol scanner_rejects_withevents_in_local_scope -- --nocapture
PASS: 1 named scanner authority test.

cargo clippy --no-deps -p oxvba-hal -p oxvba-host --all-targets -- -D warnings
PASS: focused strict Clippy for both touched crates and all their targets.

cargo clippy -p oxvba-hal -p oxvba-host --all-targets -- -D warnings
BLOCKED OUTSIDE THIS BEAD: dependency linting stops in `oxvba-bundle/src/coreir.rs:87` on the pre-existing `clippy::derivable_impls` finding for `CoreLongPtrWidth::default`.

cargo fmt --all -- --check
PASS.

cargo test -p oxvba-differential vm3_golden_snapshot
BLOCKED OUTSIDE THIS BEAD: after the `Visible` repair and the one authorized stale WithEvents row update, the first remaining drift is line 628 in `conformance/vm_package/identity_seed/vmr05_array_shape_bounds.bas`. No later row was inspected or blessed.
```

## Skips and platform boundary

- No Windows native lane was skipped: all 14 native-Declare and all 19
  string/pointer-marshalling tests executed on the Windows x64 development host.
- This bead did not execute a pinned Linux x64 CI baseline. Portable validation
  and baseline completion remain owned by `bd-59co.2.2.10` and
  `bd-59co.2.2.11`; this Windows run does not substitute for them.

## Residuals

- `vm3_golden_snapshot` remains blocked at
  `conformance/vm_package/identity_seed/vmr05_array_shape_bounds.bas`. The error
  kinds and messages match, but the stored parse offsets
  `566,587,736,736,769,769,859` differ from the current
  `587,609,765,765,799,799,892`. This bead does not guess whether source,
  normalization, or provenance changes own those offsets, and it does not alter
  that row.
- Broad strict Clippy remains independently blocked by the existing
  `oxvba-bundle` `CoreLongPtrWidth` derivable-default finding. Focused strict
  Clippy for the touched HAL and host crates is green.
- The shared public HAL code is `HAL-E-ADAPTER-FAULT`. VM execution carries the
  semantic `COM-E-DYNAMIC-NAME-UNRESOLVED` label inside `Err.Description` because
  `Fault::from_hal` does not transport the diagnostic DTO. Adding a distinct
  structured COM subcode field would be a broader versioned diagnostic-transport
  change and is not claimed here.
- No residual public diagnostic conflict remains for the touched Collection,
  native-Declare, dynamic COM-name, or local-WithEvents expectations. The later
  parse-offset drift is explicitly unresolved rather than silently blessed.
