# vm3 Completion & vm2 Retirement — Workset Plan

**Status:** Plan (design-verified 2026-06-29, two adversarial verifier passes folded in). Not yet started.
**Goal:** Finish vm3 to a **strict superset of vm2**, make vm3 the **one and only running VM** in OxVba, then **delete vm2**, refactor to a clean vm3-only codebase, and verify.
**Hard requirement:** before deletion, **no scenario where vm2 covers wider functional scope than vm3** (`vm3 ⊇ vm2`). The parity matrix below must have an empty "still vm2-wider" set.
**Authority:** live Excel VBA oracle = truth; vm3 = the executable oracle; vm2 is frozen/deprecated/known-wrong and is **not** a correctness reference — its regression role is replaced by a vm3 golden snapshot + expanded oracle before it is deleted.

## Key decisions

- **`.oxb` format changes (no sidecar, no bridge).** Replace the linearized `BundlePackage` payload with an OxIR-native package carried directly in `.oxb`: `OxImage { format: "oxvba.oxir-package", version: 1, entry: usize, programs: Vec<OxProgram> }` in `oxvba-oxir` (`OxProgram` already derives serde + carries imports/exports). A **distinct format tag** makes a stale linearized `.oxb` fail `from_bytes()` loudly. Blast radius is 4 co-built sites (build producer, comhost consumer, host engine, in-memory test ctors); the CLI and all gates run from **source**, so the format change perturbs no gate. No back-compat needed (vm2 is going away).
- **Regression-net transition (must precede deletion).** The `vm3-vs-vm2` differential gate dies with vm2, so two nets land first: **W10** expands the live-Excel oracle toward the ~254/308 corpus programs with no capture; **W11** freezes an oracle-certified vm3 **golden snapshot** (full axes: slot values, completion shape, `Err.Number/Source/Description`, `LastDllError`, instance-ids canonicalized) as the standalone regression gate. Ordering is enforced: **W10 with/before W11, both strictly before W12 (delete).**
- **Collection is prioritized** (user request): it lands at W3, right after the substrate it needs.

## The 7 functional gaps (all clean `Unimplemented` today — never wrong values)

| Gap | What | Closed by |
|---|---|---|
| G1 | built-in `Collection` (`New Collection` + Add/Item/Count/Remove) | W1 + W3 |
| G2 | `For Each` over an object (Collection + COM `IEnumVARIANT`) | W4 (Collection leg) + W5 (COM leg) |
| G3 | default-member indexing `x(i)` / `x(i)=v` (dispid-0 Item) | W4 |
| G4 | `GetObject(...)` (also absent on vm2 — net-new) | W6 |
| G5 | `WithEvents` on a COM connection-point source | W5 |
| G6 | true cross-project link (project A → project B) | W2 |
| G7 | `AddressOf` into a native `Declare` callback slot | W5 (`CallWindowProcW` synchronous slice closed by `bd-9sed.6.1`; async timer lifetime still separate) |

## Worksets (DAG; critical path W0→…→W14)

### W0 — Run-glue Ph0: `run_proc_with_values` + `activate()`/`run_entry` split
- **Goal:** value-args entry + once-per-session activation lifecycle that all downstream work needs, with single-program behavior unchanged.
- **Beads:** add `run_proc_with_values(proc, me: Variant, args: Vec<Variant>, suppress)` beside `run_proc_with_me` (vm3 lib.rs:1759); split `Vm3::run` (lib.rs:309) into `activate()` (class_descriptor leak lib.rs:314 + event_routes + global init + `reset_pending_terminations` lib.rs:362 — **once**) and `run_entry()`; `run = activate()+run_entry()`.
- **Why:** `OxArg::ByVal` wraps scalar-only `OxConst`, so host `Vec<Variant>` object/array args can't traverse `run_proc_with_me`; and the leak/reset must move out of the per-run path so a long-lived session doesn't re-leak/double-drain.
- **Verify:** existing 198-corpus single-program tests pass unchanged; unit test: 2-arg Sub via `run_proc_with_values`; assert activate-once across two invokes. **Depends on:** —

### W1 — Built-in object model as a SHARED facility (one Collection for vm2 / vm3 / JIT)
- **Design (corrected — the earlier "vm3-local `collections` + `run_native_method`" was wrong, a forbidden second truth):** the keyed Collection logic already lives VM-agnostically in `oxvba-eval` (`CollectionData`); vm2's `run_native_method` is only a thin marshalling shim. So **lift the shim into the shared crate, move the store down to the shared object box, and let vm3/JIT own only identity + minting.** No `collections` field in vm3, no `OxIR` change, no synthetic VBA `OxProgram` (vm3 reads the existing `vba_library_bundle()` metadata at runtime, as M3-1 already does).
- **W1-a [oxvba-runtime]:** move the data model (`CollectionData`/`Entry`/`Selector`/`CollectionError`) down from `oxvba-eval` into `oxvba-runtime` (needs only `Variant`, already in-crate — no cycle); add `native_state: RefCell<Option<CollectionData>>` to `CompatObjectBase` (object_ref.rs:537) + a **closure accessor** `ObjectRef::with_native_collection<R>(&self, impl FnOnce(&mut CollectionData) -> R) -> R` (lazy Default-init, `is_compat_object`-guarded — mirrors `project_field_get`; a `&mut` cannot escape the `RefMut`, so it must be closure-based, not `&mut`-returning); drop `native_state` at all three teardown points (`compat_release`:750, `finish_pending_termination`:665, `reset_pending_terminations`:688) — **kills vm2's documented Collection leak** and gives `Set c2=c1` / `Is` reference-semantics for free.
- **W1-b [oxvba-eval]:** re-export the data model (keep the `oxvba_eval::collection::*` path both VMs use) + add the shared receiver-taking dispatcher `pub enum CollectionMethod{Add,Item,Count,Remove}` + `pub fn dispatch_collection(method, data: &mut CollectionData, args: &[Variant]) -> Result<Variant, CollectionError>`, lifting vm2's `variant_selector`/`optional_key`/`optional_selector`/`required_selector` (vm2/lib.rs:3436-3473) in as private helpers; add `CollectionError::ArgNotOptional` (449) so the 449 path is data-model, not VM.
- **W1-c [oxvba-vm3]:** add the runtime-resolution substrate — `resolve_extern_class(import) -> &'static RuntimeClassDescriptor` (**synthesize + leak** `RuntimeClassDescriptor{ name:"Collection", interfaces:[IUNKNOWN] }`, mirror lib.rs:318; the bundle-level `ClassDescriptor` from `vba_library_bundle()` is used ONLY to resolve member-name+kind→`NativeMethodId`, never as the QI descriptor) + `native_method_for(class, name|default, kind) -> Option<NativeMethodId>` (reads `vba_library_bundle()` exactly as `resolve_library_import`, lib.rs:1473-1511). No `collections` field, no `OxIR` change.
- **W1-d [oxvba-vm2, cross-check]:** rewrite `run_native_method` (lib.rs:1494) to `with_native_collection` + `eval::dispatch_collection`; **delete** vm2's `collections` HashMap (lib.rs:386) + private selector helpers (3436-3473) — exactly one shim exists; the 198-row corpus + com_matrix must stay byte-identical green (proves the shared dispatch is behavior-preserving before vm3 relies on it).
- **Verify:** `cargo check -p oxvba-runtime` (no cycle); two `ObjectRef` clones share one `CollectionData` and refcount-0 clears it; eval unit tests cover keyed/457/9/449/before-after; vm2 parity unchanged. **Depends on:** — *(independent of W0; can run in parallel)*

### W2 — Multi-`OxProgram` LINK substrate + executor (closes G6)
- **Closes:** G6 cross-project link; the multi-program image the session API, `.oxb`, and comhost consume; the home for the VBA program.
- **Beads:** add `LoadedProgram` / `ResolvedImport` / `Vm3LinkError` (twins of vm2 `LoadedBundle`/`LinkError`); `Vm3::link(programs, host)` (entry=last, dup-unit reject, import resolution by `ExportToken::matches`, VBA-unit special path); refactor `Vm3` to `programs: Vec<LoadedProgram>` + `cur` cursor and rewrite the ~11 `self.program` reader sites; **widen `Loc::Global(usize)` → `Loc::Global(prog, slot)`** + `Frame.prog`/`return_prog` and sweep all `Loc` consumers incl. the `for_each` map key (lib.rs:282); implement `CallExtern` proc-body arm + `NewExtern`/`PredeclaredExtern` arms; stamp program id into `ObjectRef::from_project_instance` (replace literal 0 at lib.rs:1711/1740); make dispatch/identity/TypeOf/RaiseEvent program-aware (key `event_routes` on `(sink-prog, token, event)` to avoid cross-program token collision; `maybe_drain` indexes `programs[bundle_id]`).
- **Verify:** single-program tests unchanged; link Lib+App; unknown-unit/missing-export/dup-unit → `Vm3LinkError`; cross-program call/ByRef/method/TypeOf/WithEvents/fault-propagation/Terminate; the diamond A→B→D,A→C→D closure (mirror `oxvba-bind/tests/cross_project.rs`). **Depends on:** W0, W1

### W3 — G1 Collection wired on vm3 *(user-prioritized; needs only W1, not the linker)*
- **W3-a [oxvba-vm3]:** add the `OxInst::NewExtern` arm (currently catch-all `Unimplemented` lib.rs:1373) — resolve via W1-c, mint via `ObjectRef::from_project_instance` (mirror `new_project_instance` 1696-1717) using a **reserved out-of-band route-key sentinel** checked *before* the `program.classes` lookup (NOT `is_project_instance()` alone — that only means `compat_identity != route_key`, and a stray sentinel would still index `program.classes` → collision/Err438); `has_terminate=false`, no `Initialize`; `CollectionData` is lazily created on first use via `with_native_collection`.
- **W3-b [oxvba-vm3]:** in `dispatch_project_method` (lib.rs:1905), **before** the project class lookup, add the native-method leg gated on the Collection sentinel — resolve member→`NativeMethodId` (W1-c), marshal `OxCallArgs→Vec<Variant>` (Omitted→`MISSING_ARG`), `recv.with_native_collection(|d| oxvba_eval::collection::dispatch_collection(method, d, &argv))`, map `CollectionError→Fault` via a 5-line vm3 `collection_fault` (9/457/5/449).
- **Cleanup bead (gated AFTER W3 is green on BOTH vm2 & vm3):** delete the dead keyless first-cut — `oxvba-lib` `pure::collection_*` + its four `invoke` arms, `NativeImplId::Collection*` + `LibraryModule::Collection`, the four catalog stubs, **and** the 4 legacy fixtures (`conformance/tests/object_collection_*.bas`, `consolidate_collection_host_mix.bas`) + their `tests_manifest.csv` rows + the `run-conformance-oracle.ps1:57-60` patterns (by-name callers the first grep missed). `NativeMethodId::Collection*` (keyed) STAYS — different enum.
- **Verify:** `New Collection`/Add/Item/Count/Remove match the **live oracle** (vm3 is the oracle); `object_identity_is_same_and_different.bas` (the one `KNOWN_VM3_DEFERRED_SKIPS` entry) now **runs and matches**. **Depends on:** W1

### W4 — G3 default-member indexing + G2 Collection-leg For-Each
- **W4-a [oxvba-vm3]:** default-member `coll(i)` at the `ArrayGet` object arm (lib.rs:954, `Unimplemented`) — Collection receiver → resolve default member (`is_default_member` Item, vba_library.rs:108) → `CollectionItem` via `dispatch_collection` → store. **`coll(i)=v` at `ArraySet` (lib.rs:978) must RAISE** the error the live oracle gives (the built-in Collection is read-only through its default member) — **do not silently no-op**; probe the exact `Err` number against the oracle. Keep project/COM default-member legs distinct.
- **W4-b [oxvba-vm3]:** For-Each at the `ForEachInit` object arm (lib.rs:1033) — Collection receiver → snapshot `with_native_collection(|c| c.values())` (insertion order; supports mid-loop `Exit For`); project-instance-without-collection → empty; genuine COM object → the existing host `IEnumVARIANT` leg (W5).
- **Verify:** `c(1)`/`c("key")` read; the assignment form errors exactly as the oracle does; For-Each yields insertion order vs the oracle. **Depends on:** W3

### W5 — COM-foreign legs: G2-COM (`IEnumVARIANT`) + G5 (`WithEvents`) + G7 (`AddressOf`→native)
- **Note:** the HAL/`oxvba-com` stack already exists and is live-tested under vm2 — this is mostly vm3 wiring, plus one shared facility for G7.
- **Beads:** **G2-COM** — replace the For-Each object-arm `Unimplemented` (lib.rs:1034) with vm2's branch (`host.com().enumerate_object(obj)`); **G5** — port vm2's COM-event model (`com_subscriptions` maps + `subscribe/pump/unsubscribe_com_*`, vm2/lib.rs:1540-1644), replace `WithEvents`-COM `Unimplemented` (lib.rs:1195), deliver handler args via W0 `run_proc_with_values`, pump at statement boundaries, unsubscribe on `Set=Nothing`/teardown/Terminate; **G7** — DONE for the bounded synchronous `CallWindowProcW(AddressOf ...)` shape by `bd-9sed.6.1`: `oxvba-runtime::callback_thunks` provides a VM-agnostic scoped callback-thunk facility (32-slot thread-local, opaque proc token + `CallbackExecutor`, no vm2 dependency, macro-generated `extern "system"` trampolines in `catch_unwind`, dedup by `(owner, token)`, Err 7 on exhaustion), and vm3 wires the `Declare` LongPtr+ProcRef arm to register a slot for the duration of the native call.
- **Verify:** live com_matrix For-Each-over-COM + WithEvents-COM (V7/V8, in-proc + OOP arg order) green; the synchronous `CallWindowProcW` callback fires the VBA proc with no host AV; `cargo check -p oxvba-runtime` names no `Vm` type; Miri-style review of the unsafe trampoline. Async `SetTimer`/message-pump callbacks remain separate native-lifetime work. **Depends on:** W0, W2, W4
- **Evidence 2026-07-02 (`bd-9sed.6`):** scoped W5 live rows are green on vm3. See `docs/evidence/com/VM3_W5_COM_FOREIGN_LEGS_2026-07-02.md` for the exact command set. Residual async/native callback lifetime work is tracked separately as `bd-9sed.17`; V11 remains a documented fixture gap for ByRef COM event arguments rather than a hidden parity claim.

### W6 — G4 `GetObject` (net-new; vm2 also lacks it → superset-for-free)
- **Beads:** front-end — `NativeImplId::GetObject` + symbol-catalog entry (SpecialForm, both args optional) + binder lowering mirroring `CreateObject`; HAL/host/com — `ComHal::get_object_variant(pathname, class)` (default = capability error) + standard-adapter impl + `oxvba-com activate_dispatch_get_object` (`GetActiveObject`/ROT for running instance; `CoGetObject`/`MkParseDisplayName`+`BindMoniker` for file/moniker) + `oxvba-lib host::get_object`; 1-arg no-instance → clean `Err 429`; moniker form reuses the M3-8 rich HRESULT→Err machinery; null/wasm/replay decline cleanly.
- **Verify:** `catalog_covers_every_native_impl_id` passes; live `GetObject(,"Excel.Application")` / `GetObject("c:\book.xlsx")`; null adapter returns a clean error not a panic. **Depends on:** W5 (can run beside W3/W4/W5 — only feeds W8)
- **Evidence 2026-07-02 (`bd-9sed.7`):** W6 is green on vm3. See `docs/evidence/com/VM3_W6_GETOBJECT_2026-07-02.md` for exact commands and live GetObject coverage. Unsupported Null/Replay COM activation now declines via the normal capability-unavailable path, not the generic variant-companion fallback.

### W7 — Session API Ph1/Ph2 + host vm3-backed `ProjectRuntimeSession`
- **Closes:** PM-2 run-glue parity.
- **Beads:** vm3 — `project_event_sink` field + `set/clear_project_event_sink` (invoked after RaiseEvent fan-out lib.rs:1258); public `create_project_instance(prog, class_name)` + `invoke_member_values(obj, name, kind_hint, Vec<Variant>)` (twins of vm2/lib.rs:572/640/697/704); host — re-back `ProjectRuntimeSession`'s inner VM to vm3 (same Box::leak-to-`'static`), add `prepare_oxir_package_session(OxImage)`, re-implement `create_class_instance`/`invoke_member_values`/`set+clear sink` on vm3, leave `bind_native_dispatch_object_value` (host-services) unchanged, **add the missing `if enable_jit { return jit_not_implemented }` guard** to `prepare_oxir_package_session` and every `execute_*_vm3`. Keep `prepare_bundle_package_session` (vm2) compiling **until W12**.
- **Verify:** create+invoke with `Vec<Variant>` args; sink fires once per RaiseEvent; lifecycle reset not per-invoke; `package_session_events` re-pointed and green on vm3. **Depends on:** W2, W5

### W8 — `.oxb` → `OxImage` + product flip (build → host → comhost → CLI) — *vm3 becomes the sole VM*
- **Beads:** add `OxImage` + tag/version + `to_bytes/from_bytes/validate` + `OxImageError` to `oxvba-oxir`; **add a cross-format-rejection test** (a serialized `oxvba.bundle-package` stream must fail `OxImage::from_bytes` with `UnsupportedFormat`); `oxvba-build` — `build_oxir_package(closure) → OxImage` and swap it into `build_wrapped_com_server` (lib.rs:119-121); `oxvba-comhost` — `OxImage::from_bytes` + `prepare_oxir_package_session` at `with_session` (lib.rs:2993/3000), **reject `programs.len()>1` with a precise diagnostic until multi-program comhost is proven** (W2-backed); **CLI/host — enumerate and flip ALL six vm2-only host executors** (`prepare_bundle_package_session`:502, `execute_project_closure_with_variant_snapshot`:536, `execute_source_with_variant_snapshot_clean`:571 = the real `oxvba run` entry main.rs:106, `execute_source_with_references_and_snapshot`:590, `execute_manifest_with_variant_snapshot`:621, `execute_manifest_snapshot_with_err`:718) — give each a vm3 twin or an explicit delegation; add `execute_source_with_references_and_snapshot_vm3` (the missing early-bound-COM-from-CLI leg); closure entry → `Vm3::link`; map `Vm3Snapshot::Unsupported` → `RUN-E-VM3-UNIMPLEMENTED` (never a silent skip) and `--jit` → `RUN-E-JIT-NOT-IMPLEMENTED` + exit 1.
- **Verify:** `OxImage` round-trip + stale-tag rejection; build emits a vm3-loadable `.oxb`; comhost live legs green on vm3; `oxvba run <src> --dump-values` matches prior vm2 VALUES (modulo known-wrong-vm2 corrections); `oxvba run --jit` exits 1. **Depends on:** W6, W7

### W9 — PARITY PROOF (licenses deletion)
- **Beads:** run the full 308-program differential corpus on vm3 from source; **assert `in-scope-skipped == 0` (the `skip_reasons` bucket empty) AND `mismatches == 0` AND `KNOWN_VM3_DEFERRED_SKIPS == []`** (the one deferred file now runs+matches via W3); walk the parity matrix row-by-row and confirm `residual_vm2_wider` is empty (G4 both-lack and G7 thin-coverage are the only documented non-blocking residuals).
- **Verify:** full-workspace tests; differential in-scope-skip==0 & mismatches==0; live com_matrix 5/5+; oracle 100% in-scope. **Depends on:** W8

### W10 — Regression-net: live-Excel ORACLE expansion *(with/before the golden freeze)*
- **Beads:** capture oracle results for the ~254/308 corpus programs with no current capture (fresh host — heed the "wedged COM procs need reboot" hazard); commit captures; extend `oracle_conformance` to assert vm3 matched==total outside a strictly-shrinking allowlist; add fresh-host captures for the new W5/W6 COM/GetObject legs.
- **Verify:** vm3 100% oracle-compliant in-scope on the enlarged corpus. **Depends on:** W9

### W11 — Regression-net: vm3 GOLDEN snapshot gate *(must land before deletion)*
- **Beads:** freeze vm3's oracle-certified full-axis snapshot for every corpus program; add a standalone golden test that **replaces** `vm3_matches_vm2_across_the_corpus_subset`; prove the net bites (a deliberate one-op perturbation flips a golden).
- **Verify:** golden gate green over the full corpus; perturbation flips a golden. **Depends on:** W10

### W12 — DELETE crate `oxvba-vm2` + re-point all dependents
- **Build-critical closure (atomic, in this order, so `cargo test --workspace` is green at the commit boundary):** (1) re-point `oxvba-bind`'s 3 dev-dep roundtrip tests (`bind_roundtrip.rs`, `cross_project.rs`, `feature_coverage.rs`) to `bind→elaborate→Vm3`; (2) **re-point the broader host-test set that calls a vm2 executor** — `clean_path_closure.rs`, `riff_external_corpus.rs`, `sqliteforexcel_declare_integration.rs`, `com_office_integration.rs`, `vba_web_external_corpus.rs`, `com_matrix_common.rs`, `debug_and_console_print.rs`, `filesystem_statements.rs`, `native_declare_lane.rs`, `native_declare_string_marshalling_end_to_end.rs`, `package_session_events.rs` — to the vm3 executors; (3) remove `oxvba-differential` `Executor::Vm2` + match arms (lib.rs:210/245) + `KNOWN_VM2_DIVERGENCES` (:698) + `is_tolerated_vm2_divergence` (:724) + the dead `vm3_matches_vm2*` tests + the `oracle_conformance.rs` (**test file**) vm2 column; (4) remove host `engine.rs` vm2 methods + `ProjectRuntimeSession` vm2 arm + `runtime_diagnostic(VmError)`; (5) **delete `crates/oxvba-vm2`** + drop the workspace member and the `oxvba-host`/`oxvba-bind` Cargo dep lines.
- **Verify:** `cargo build/test --workspace` green (golden W11 + oracle W10 carry regression coverage); `git grep -lE 'oxvba_vm2|oxvba-vm2' -- '*.rs'` empty. **Depends on:** W9, W10, W11

### W13 — REFACTOR to a clean vm3-only codebase
- **Beads:** **surgical `oxvba_bundle` split** — KEEP coreir + manifest contracts (`BundleExport/Import`, `ComClassExport`, `EventRoute`, `ExternalCallDescriptor`) + `native.rs` ids (shared with oxir/vm3); REMOVE only `linearize` + the Op-array `Bundle` *instruction* form + `BundlePackage`; **KEEP `vba_library.rs` and the metadata shape `vba_library_bundle()` needs** (Collection class descriptor + native-method ids + library exports) — vm3 reads it at runtime, so the `Bundle`-type removal must preserve that descriptor/export metadata even as the linearized Op *instruction array* goes; collapse any `{Vm2|Vm3}` session enum to vm3-only; re-anchor the ~60 "mirrors vm2"/"matches vm2" comments (≈60 in vm3/lib.rs) to the oracle+golden authority; **re-anchor the comment-only vm2 references** in `oxvba-eval/lib.rs`, `oxvba-lib/lib.rs`, `oxvba-project/lib.rs`, `oxvba-cli/main.rs`, **and `oxvba-bundle/src/native.rs:3` + `oxvba-bundle/src/lib.rs:12`** (the two the first grep missed); re-home `ERROR_CODES.md` RUN-E-* authority to vm3; fix the dangling `cargo test -p oxvba-vm2 --test linearize_roundtrip` in `docs/spec/OXVBA_REPRESENTATION_LAYOUT_DOCTRINE_V1.md:108` and the ~9 vm2-naming `.md` files.
- **Verify:** workspace build/test; `git grep -n 'mirrors vm2|matches vm2'` returns only intentional historical prose; coreir+contracts+collection still compile and are used by oxir/vm3. **Depends on:** W12

### W14 — POST-DELETION VERIFY
- **Beads:** `cargo build/test/clippy --workspace` clean (incl. golden W11 + oracle W10); live com_matrix legs green; **`git grep` clean for `oxvba_vm2`/`oxvba-vm2`/`BundlePackage`/`linearize` over BOTH `*.rs` AND `*.md`** (modulo intentional historical prose — which does NOT cover stale architecture text in live `.rs` module docs).
- **Exit:** vm3-only workspace verified green end-to-end with no dangling vm2/old-format references. **Depends on:** W13

## Parity matrix (must end empty of "vm2-wider")

| Capability | vm3 today | Closed by |
|---|---|---|
| PM-1 Core IR vocabulary (value/control/calls/error/arrays/records/objects/events/Declare/typed-COM) | **at parity** (M3-10 floor) | already closed |
| PM-2 run-glue / session+activation API | vm2-wider | W0 + W7 |
| PM-3 built-in object model (Collection, default-member, Collection For-Each) | vm2-wider | W1 + W3 + W4 |
| PM-4 cross-project link + executor | vm2-wider (G6) | W2 |
| PM-5 COM-foreign legs (For-Each-COM, WithEvents-COM, AddressOf-native) | vm2-wider | W5 |
| PM-6 `GetObject` | neither runs it | W6 (net-new) |
| PM-7 `.oxb` artifact + product flip | vm2-only | W8 |

**Residual vm2-wider after the plan: none.** (G4 is net-new on both; G7 closes but has thin corpus coverage — flagged.)

## Top risks
- **W1 native-body-carrier fork** (IR field vs side table) — settle before W2 to avoid rework; the synthetic VBA `OxProgram` must export/import identically to `vba_library_bundle` or the built-in-function differential regresses.
- **W2 `bundle_id`-as-program-id + `Loc::Global` widening** — one missed mint/consumer site silently mis-dispatches across programs; sweep exhaustively; cover with cross-program method/identity/Terminate tests.
- **W5/G7 `no *mut Vm` thunk** — re-entrancy + aliasing across FFI need a Miri-style review; G7 is the weakest-covered closed gap (no corpus program).
- **W10→W11 ordering is load-bearing** — the golden freezes vm3's current output as truth, so oracle expansion must certify the baseline first (enforced by the `W11 depends_on W10` edge); both precede W12.
- **W8 comhost flip** — process-lifetime cdylib with thread-local SESSION and wedge-on-crash behavior; sequence host-engine (W7) strictly before comhost (W8) and reject `programs.len()>1` until multi-program link is proven live.
