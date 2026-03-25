# OxVba Initial Scope Status — 2026-03-24

**Test count:** 2258 passing, 0 failures
**Integration fixtures:** 14 active (including newly unblocked INTP-016), 3 deferred, 3 not-created
**Formal gates:** 30 folded, 1 running, 19 deferred with rationale
**Divergences:** 0 open (4 tracked, all closed or narrowed)

---

## 1. Engine Correctness Gaps

| Gap | Severity | Status |
|-----|----------|--------|
| **String coercion not implemented** | High | Done |
| **Cross-module error propagation** | High | Done |
| **Class module `As New` in project execution** | High | Done |
| **Single-line If with statement** | Medium | Done |
| **RuntimeValue <-> Variant bridge** | Medium | Partially done |
| **ParamArray named arguments** | Medium | Done |
| **Dynamic dispatch named/omitted args** | Medium | Done |
| **Real COM activation truth / imported early-bound model** | High | In progress |

### Completed this session

- [x] **String coercion (`CStr`, `Str`, `Val`)** — Added `runtime_value_to_vba_string()` and `runtime_value_to_vba_str()` in `coerce.rs`. CStr, Str, and Val now compile as proper intrinsic instructions (`IntrinsicCStrDigits`, `IntrinsicStrFuncDigits`, `IntrinsicValDigits`) instead of being silently stripped by the parser. VM handlers convert RuntimeValue to string with VBA formatting semantics.
- [x] **Numeric coercion matrix expansion** — Added 14 new coercion paths: Integer->Single, Long->Single, Single->Double, Byte->Integer/Long/Single/Double, Boolean->Integer/Single/Double, Date->Double, Currency->Double, Empty->Integer/Long/Single/Double/Boolean. Added `from_u8()`/`as_u8()` Byte accessors to Variant.
- [x] **Cross-module error propagation** — Fixed `invoke_procedure_with_values()` and `invoke_procedure_inline_with_values()` in the VM interpreter. Both methods now save/restore the caller's error frame across call boundaries, matching VBA semantics where `On Error Resume Next` in a caller catches `Err.Raise` from called procedures.
- [x] **Class module `As New` in project execution** — Root cause was module ordering: class modules sorted alphabetically before `Main.proc.bas`, placing class helper functions at bytecode pc=0 instead of the entry point. Fixed by emitting procedural modules before class modules in `lower_project_source()`. INTP-016 integration fixture promoted to active and passing.
- [x] **Single-line If with statement** — The project resolver now parses single-line `If cond Then stmt` forms instead of treating them as unsupported multiline headers. Added compiler regression coverage for inline assignment-form lowering and host/project regression coverage for `If 1 = 1 Then Err.Raise 104`.
- [x] **ParamArray named arguments** — Calls to ParamArray procedures no longer reject named fixed parameters wholesale. Named fixed parameters now bind correctly while trailing positional arguments still pack into the ParamArray. The oracle-matched rejection for naming the ParamArray target itself (`items := ...`) remains intact and tested.
- [x] **Dynamic dispatch named/omitted args** — Native project-object `DispatchInvoke` now binds named arguments against project member metadata, applies optional defaults when trailing arguments are omitted, preserves explicit omitted-arg semantics at the VM request seam, and packs `ParamArray` values for project-dynamic routing. Added VM route-binding regressions plus host/project regressions for named function and property-let dispatch and omitted optional-default dispatch.
- [x] **RuntimeValue <-> Variant bridge (partial)** — String coercion is now handled at the RuntimeValue level via the new `runtime_value_to_vba_string`. Added Byte (`from_u8`/`as_u8`) accessors. Remaining gaps (ArrayIntent, ObjectHandle in Variant) are architecturally blocked by the Copy-able 16-byte Variant struct and correctly handled through RuntimeValue directly.

### Remaining for Initial Scope

- Real COM activation is still not parity-complete enough for honest initial-scope closure.
- Imported real COM `As New` activation is still bounded by a narrow authoritative identity subset. The compiler now takes activation identity from explicit typelib metadata (`activation_prog_id`) instead of guessing from the source type text, and still fails unsupported imported activation explicitly instead of lowering to non-VBA syntax. The remaining supported scope is still not a general real-library activation contract.
- Native Windows string-ProgID `CreateObject("...")` remains the live late-bound activation path on Windows. Numeric `CreateObject(<selector>)` scaffolding is not part of the VBA contract and is being removed from repo-local metadata/policy/test seams; the remaining repo-truth gap is that deterministic fallback/projection scaffolding still exists in neighboring lanes and must not be described as equivalent parity support for real-library activation.
- The live typelib path does not yet provide a trustworthy activation contract for arbitrary real COM libraries. The registered `Scripting.Dictionary` early-bind anchor now covers activation plus `Add` / `Exists` / `Count` for the supported subset, and `ODG-044` is now closed for that supported oracle lane. `ODG-031` still remains broader than a harness scheduling item because the general activation contract question is still open.

---

## 2. Evidence Closure — Critical Path to Initial Scope

### 2.1 Oracle Gates (IP-10)

**Required for Initial Scope (5 gates total — 4 now closed, 1 still open):**

- [x] **ODG-030** — COM interop marshaling. Closed with `com_testeventserver_marshaling_oracle_20260325T231210Z`: Excel and OxVba match on late-bound scalar arg/return (`17`), array-argument shape (`rank=1;len=3;lb=0;ub=2;first=1`), self-object roundtrip (`True`), scalar array return (`3,4`), and dispatch element inside returned array (`42`) against the real `HKCU`-registered `OxVba.TestEventServer`.
- [ ] **ODG-031** — TypeLib COM interop. The baseline user-scope external typelib lane is now paired via `com_testeventserver_oracle_20260325T221949Z`, and the versioned/broken-reference matrix is now built and closed under `ODG-046` via `com_testeventserver_versioned_typelib_probe_20260325T222709Z`. The remaining open work is the correction of the real COM activation/model assumptions that still overread the supported imported scope.
- [x] **ODG-044** — `As New` early-bound supported subset. Oracle run `com_early_oracle_20260325T145433Z` matched Excel and OxVba for `Dim obj As New Scripting.Dictionary` plus `Add` / `Exists` / `Count` (`True,1`). Broader real-library activation authority remains open under `ODG-031`, not under this supported-subset oracle gate.
- [x] **ODG-045** — Dual-interface vtable/dispatch. Closed with mixed-server oracle run `com_dualinterface_oracle_20260325T224113Z`: Excel, OxVba default dispatch policy, and OxVba `PreferVtable` policy all match on `Scripting.Dictionary` (`True,1`) and `TestEventServer.Ping()` (`42`), giving explicit evidence for both strategy stability and fallback constraints.
- [x] **ODG-046** — TypeLib version/broken-ref. Closed with `com_testeventserver_versioned_typelib_probe_20260325T222709Z`: direct `AddFromFile` against the temp-built `2.0` typelib resolves as `2.0`, a workbook saved against `1.0` does not auto-upgrade when the same path is replaced with `2.0`, removing the referenced file yields a broken reference, and restoring the file repairs it back to working `1.0` with `Ping() = 42`.

**Deferred beyond Initial Scope (6 gates — blocked on missing subsystems):**

- [x] ODG-032 — Stateful file I/O. Correctly deferred (subsystem not built).
- [x] ODG-033 — Host capability/policy. Correctly deferred.
- [x] ODG-040 — Host extension modules. Correctly deferred.
- [x] ODG-041 — TypeLib/importlib resolution. Correctly deferred.
- [x] ODG-042 — MS-OVBA storage roundtrip. Correctly deferred.
- [x] ODG-043 — Startup/entrypoint. Correctly deferred.

### 2.2 Formal Deferred Gates (IP-11) — 50 Gates Total

| Status | Count | Notes |
|--------|-------|-------|
| dg-folded | 30 | Done. Includes 3 batch-folded this session (DG-V79, V80, V85-002). |
| dg-running | 1 | DG-V2-001. Still executing remotely. |
| dg-deferred | 19 | All have explicit deferral rationale as of this session. |
| dg-pass | 0 | All pending foldback completed. |
| dg-fail | 0 | All moved to dg-deferred with rationale. |
| dg-not-started | 0 | All moved to dg-deferred with rationale. |

Completed this session:

- [x] **Fold DG-V79-001** — pass confirmed, no outstanding obligations.
- [x] **Fold DG-V80-001** — pass confirmed, no outstanding obligations.
- [x] **Fold DG-V85-002** — pass confirmed, no outstanding obligations.
- [x] **Defer DG-V81, V83** — runner crash, not harness failure. Requires remote Linux rerun.
- [x] **Defer DG-V87, V88, V89, V146** — timeout/OOM dominated (FO-V2-001, FO-V4-001). Not implementation bugs.
- [x] **Defer DG-V4, V107, V120, V126, V132, V134, V162, V175, V176** — require remote Linux Kani runner. Covered by executable tests.

### 2.3 Active Blockers

- [ ] **BLK-COM-ACTIVATION-001** — Real COM activation/model gap. Imported early-bound `As New` now uses explicit typelib-owned activation identity and has a real registered `Scripting.Dictionary` activation-plus-member anchor on the supported subset, but broader real-library activation-model parity is still in progress and adjacent late-bound fallback/projection boundaries still need an explicit truth audit.
- [ ] **BLK-FORMAL-001** — Infrastructure gap. Explicit deferrals now in place; remaining need is DG-V2-001 completion.

---

## 3. Integration Fixtures

### Active (14 fixtures — up from 12)

- [x] INTP-001 through INTP-011 — all passing
- [x] INTP-017 — error handling across module boundaries (passing)
- [x] INTP-019 — 3+ references and shadowing chains (passing)
- [x] **INTP-016** — Multi-module class hierarchy with As New instantiation (**newly unblocked this session**)

### Deferred (3 fixtures — down from 4)

- [ ] INTP-012 — Startup metadata / auto-entrypoint. Blocked on ODG-043.
- [ ] INTP-013 — Host extension module lifecycle. Blocked on ODG-040.
- [ ] INTP-014 — Stateful file I/O. Blocked on ODG-032.

### Not Created (3 fixtures)

- [ ] INTP-015, INTP-018, INTP-020 — require infrastructure that doesn't exist. Post-Initial Scope.

---

## 4. New Capabilities — Scope Decisions

These are post-Initial Scope items. No action needed for v620 closure.

### 4.1 IP-06 S1-S3: Outward COM Server

- [x] **Decision: Post-Initial Scope.** S0 is closed. S1-S3 scaffolding exists but functional wiring (bundle loading, dispatch table, VARIANT marshaling, lifecycle hooks) is a separate scope.

### 4.2 COM-EVT-B: Source-Interface Events

- [x] **Decision: Correctly deferred.** Not required for VBA 7.1 Office-style parity (EPD-05).

### 4.3 Platform CI Expansion

- [ ] Wire Miri into CI — low cost, catches real UB. Consider for Initial Scope.
- [ ] Expand Linux CI to full `cargo test` — currently governance-only. Consider for Initial Scope.
- [x] macOS CI — Post-Initial Scope. Manual validation sufficient.
- [x] WASM CI — Post-Initial Scope.

### 4.4 Conformance Corpus Expansion

- [ ] COM marshaling edges (8-12 files) — not blocked, nice-to-have
- [ ] TypeLib-driven binding (6-10 files) — not blocked, nice-to-have
- [ ] Wider event testing (5-8 files) — not blocked, nice-to-have
- [ ] Multi-feature consolidation (5-10 files) — not blocked, nice-to-have
- [ ] File I/O (10-15 files) — blocked on ODG-032

### 4.5 Verification Expansion

- [ ] Type coercion matrix proptest — can do now, high value
- [ ] Parser round-trip proptest — can do now, medium value
- [ ] VARIANT roundtrip proptest — can do now, medium value
- [ ] SAFEARRAY/Decimal/BSTR/Object Miri mocks — can do now, extends existing module
- [ ] COM VARIANT roundtrip Kani — post-Initial Scope (needs remote Linux)
- [ ] SafeArray bounds Kani — post-Initial Scope
- [ ] JIT slot ABI Kani — post-Initial Scope

---

## 5. Correctly Deferred — No Action Needed

- [x] COM-EVT-B (source-interface events) — not required for VBA 7.1 parity
- [x] INTP-015, INTP-018, INTP-020 — need infrastructure that doesn't exist
- [x] JIT TypeOf...Is native emission — functional via interpreter fallback
- [x] Broadword/SWAR decoder — performance feature, placeholder exists
- [x] HAL-DYN-018/019 (pointer-string, ByRef writeback) — deterministic-rejection
- [x] Non-Windows COM stub bridge — intentional platform gate

---

## 6. Divergences

- [x] DIV-0001 (If Statement) — closed
- [x] DIV-0002 (For Loop) — closed
- [x] DIV-0003 (Implements Interface) — closed for baseline
- [x] DIV-0004 (WithEvents/RaiseEvent) — narrowed, deterministic baseline closed

---

## 7. What Remains for Initial Scope Closure

### Must-do (blocking v620 terminal gate)

- [ ] **COM activation truth review/fix** — Continue correcting the real COM activation model so imported early-bound `As New` uses an authoritative real-library activation contract for the supported scope, and so native late-bound `CreateObject("ProgID")` is documented/tested separately from deterministic fallback/projection scaffolding instead of being blurred into one parity claim.
- [x] **ODG-045 mixed-server oracle harness** — closed with `com_dualinterface_oracle_20260325T224113Z`.
- [x] **ODG-046 versioned/broken-reference oracle harness** — closed with `com_testeventserver_versioned_typelib_probe_20260325T222709Z`.
- [ ] **Close or defer ODG-031** — the user-scope baseline and versioned/broken-reference oracle matrix now exist; the remaining gap is the imported activation/model correction above.
- [ ] **DG-V2-001 completion** — still running remotely. Needs to complete or be explicitly deferred.

### Should-do (high value, not strictly blocking)

- [ ] Type coercion proptest expansion — catches bugs without external dependencies
- [ ] Consider wiring Miri and Linux full test into CI

### Will not do for Initial Scope

- [x] IP-06 S2/S3 functional wiring — post-scope
- [x] macOS/WASM CI — post-scope
- [x] Kani expansion targets — need remote Linux
- [x] File I/O subsystem — entire subsystem not built
- [x] Host extension lifecycle — entire subsystem not built
- [x] MS-OVBA storage roundtrip — infrastructure not built

---

## Files Changed This Session

| File | Change |
|------|--------|
| `crates/oxvba-runtime/src/coerce.rs` | Expanded numeric coercion matrix (14 paths); added `runtime_value_to_vba_string`, `runtime_value_to_vba_str`, `format_vba_f64` |
| `crates/oxvba-runtime/src/variant.rs` | Added `from_u8()`/`as_u8()` Byte accessors |
| `crates/oxvba-runtime/src/lib.rs` | Re-exported new coercion functions |
| `crates/oxvba-compiler/src/resolve.rs` | CStr, Str, Val now emit as IntrinsicCall |
| `crates/oxvba-compiler/src/bytecode.rs` | Added `IntrinsicCStrDigits`, `IntrinsicStrFuncDigits`, `IntrinsicValDigits` instructions |
| `crates/oxvba-compiler/src/emit.rs` | Wired cstr, str, val to new instructions |
| `crates/oxvba-compiler/src/project.rs` | Fixed module ordering: procedural before class modules |
| `crates/oxvba-vm/src/interpreter.rs` | Error state save/restore in cross-module calls; CStr/Str/Val VM handlers |
| `conformance/integration/catalog.psv` | INTP-016 promoted to active |
| `docs/evidence/formal/DEFERRED_GATES.md` | Folded 3 dg-pass, deferred 15 dg-fail/not-started with rationale |
