# RuntimeValue Active-Use Inventory (2026-05-01)

Status: evidence for bead `bd-pn5i.2` / `cleanout-001`

Scope: active Rust crates plus non-archived docs. Archived docs excluded from the
inventory scan are `docs/archive/**` and `docs/spec/archive/**`.

## Scan Evidence

Commands run from repo root:

```bash
rg -n "\bRuntimeValue\b" crates docs --glob '!docs/archive/**' --glob '!docs/spec/archive/**'
rg -l "\bRuntimeValue\b" crates | wc -l
rg -o "\bRuntimeValue\b" crates | wc -l
rg -l "\bRuntimeValue\b" docs --glob '!docs/archive/**' --glob '!docs/spec/archive/**' | wc -l
rg -o "\bRuntimeValue\b" docs --glob '!docs/archive/**' --glob '!docs/spec/archive/**' | wc -l
rg -n "CfgIr|VbaHir|VbaMir" crates docs --glob '!docs/archive/**' --glob '!docs/spec/archive/**'
```

Results:

- `RuntimeValue` in crates: 63 files, 3066 occurrences.
- `RuntimeValue` in non-archived docs: 270 files, 1068 occurrences.
- Combined non-archived scope: 333 files, 4134 occurrences.
- Fake IR terms (`CfgIr|VbaHir|VbaMir`) now appear only in current explanatory
  docs/worksets:
  - `docs/IR_DESIGN.md`
  - `docs/worksets/WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md`
  - `docs/worksets/WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`

Top active code/doc concentration from the count script:

- `crates/oxvba-host`: 21 files, 1188 occurrences.
- `crates/oxvba-hal`: 17 files, 708 occurrences.
- `crates/oxvba-vm`: 4 files, 660 occurrences.
- `crates/oxvba-runtime`: 6 files, 228 occurrences.
- `crates/oxvba-jit`: 4 files, 185 occurrences.
- `crates/oxvba-com`: 7 files, 83 occurrences.
- `docs/evidence/value_model_migration/**`: 236 files, 771 occurrences, most
  of them historical run logs/evidence notes.

## Inventory Families

| ID | Family | Classification | Representative surfaces | Current role | Deletion / migration path | Owner / blocker disposition |
|---|---|---|---|---|---|---|
| RV-01 | Runtime carrier enum and public re-export | public API, projection | `crates/oxvba-runtime/src/runtime_value.rs`, `crates/oxvba-runtime/src/lib.rs` | Defines `RuntimeValue` plus reusable scalar/handle helper types in the same file. | Extract still-needed helper types (`F64Value`, `CurrencyValue`, `BindingHandle`, `DynLinkSymbol`) from the legacy carrier module, stop re-exporting `RuntimeValue`, then delete or quarantine the enum. | `bd-pn5i.5`; no blocker yet, but it must land after VM/JIT/HAL/COM/host callers stop requiring the enum. |
| RV-02 | Variant/RuntimeValue bridge helpers | helper, projection | `RuntimeValue::{to_variant,from_variant}`, `Variant::{try_from_runtime_value,from_runtime_value,to_runtime_value}` | Explicit compatibility bridge between retained `Variant` and legacy values. | Replace call sites with direct `Variant` constructors/accessors. If any temporary bridge remains, move it into one named compatibility module with a removal note before search-gate closure. | `bd-pn5i.5`; residual bridge is not acceptable outside an approved blocker module. |
| RV-03 | SAFEARRAY legacy value APIs | public API, helper, tests | `SafeArray::{from_values,from_values_nd,from_typed_values*,elements,replace_elements}` | Compatibility APIs over retained `Variant` SAFEARRAY payloads. | Migrate callers/tests to `from_variants`, `from_typed_variants*`, `variant_elements`, and `replace_variant_elements`; delete legacy constructors/accessors once callers move. | `bd-pn5i.5`, with VM/JIT caller updates in `bd-pn5i.3`/`bd-pn5i.4`. |
| RV-04 | Pointer helper legacy APIs | helper, boundary projection | `register_runtime_value_pointer`, `register_string_var_pointer`, `register_variant_var_pointer`, `register_object_pointer`, `register_legacy_*`, `read_back_*_payload` | Retained pointer helpers already have `Variant` entry points, but legacy wrappers still expose `RuntimeValue`. | Route VM/JIT/host pointer paths through `Variant` helpers; delete or isolate legacy wrappers after tests are migrated. | `bd-pn5i.5`; object/binding-token compatibility must be named if it survives. |
| RV-05 | Runtime string/coercion helpers | helper, stale comments, tests | `runtime_value_to_vba_string`, `runtime_value_to_vba_str`, coerce comments/tests | Some string/coercion helpers still return `RuntimeValue` despite existing `variant_to_vba_string` support. | Use `Variant`/`BStr` return shapes for runtime helpers; move mixed numeric/string coercion details into the value-substrate cleanup matrix where behavior is broader than mechanical type removal. | `bd-pn5i.5` for type removal; phase-3 `value-clean-001` for broader numeric/coercion helper migration. |
| RV-06 | VM slots, snapshots, invocation, and semantic helpers | execution, helper, tests | `crates/oxvba-vm/src/register_file.rs`, `interpreter.rs`, `semantics.rs`, `lib.rs::compat` | VM storage is retained `Variant`, but legacy snapshots, invocation APIs, test helpers, and many semantic helper functions still expose or build `RuntimeValue`. | Make retained `Variant` APIs the only normal VM execution surface; convert tests to `Variant` assertions or explicit DTO/projection checks; migrate/delete legacy helper functions as corresponding `Variant` companions become authoritative. | `bd-pn5i.3`; broad numeric helper semantics feed phase-3 `value-clean-001`. |
| RV-07 | Host engine/session/event public compatibility APIs | public API, execution, projection, tests | `Engine::{invoke_procedure,invoke_member_on_object,execute_*_with_snapshot}`, `ProjectRuntimeSession::{snapshot,read_slot}`, `ComEventCallbackDispatch`, `crates/oxvba-host/src/compat.rs` | Host product paths have `Variant` companions, but legacy APIs and tests still traffic in `RuntimeValue`. | Promote `*_variants` APIs and delete/DTO-split legacy `RuntimeValue` API shapes; migrate event callback payloads to retained `Variant` or explicit host DTOs. | `bd-pn5i.3`; host presentation/value DTOs discovered here should not be left only under launcher/web cleanup. |
| RV-08 | Host observation DTOs: immediate/debugger/embedded | DTO/projection, public API, tests | `crates/oxvba-host/src/immediate.rs`, `debugger.rs`, `embedded.rs` | User-facing/debug/embedded result structs retain `RuntimeValue` fields beside existing or emerging `Variant` forms. | Split stable display/JSON/result DTOs from semantic carriers; retained execution state should expose `Variant`, while UI/IDE surfaces expose display text/kind-specific DTOs. | `bd-pn5i.3` for host-side migration; `bd-pn5i.6` for downstream web/language-service DTO consumers. |
| RV-09 | HAL trait contracts and adapters | public API, execution boundary, helper | `crates/oxvba-hal/src/traits.rs`, `compat.rs`, `adapters/{standard,null,wasm,recording,replay}.rs` | HAL traits still require legacy `RuntimeValue` methods even though `_variant` companions are the retained VM/JIT entry points. | Invert or split traits so `Variant` methods are primary. Delete or quarantine old methods/adapters after callers/tests migrate. Recording/replay journal payloads need explicit DTO/schema treatment if retained. | `bd-pn5i.5`; no current external blocker identified. |
| RV-10 | COM model and Windows bridge compatibility | boundary projection, public API, helper, tests | `crates/oxvba-com/src/model.rs`, `compat.rs`, `dynamic_object.rs`, `windows_variant.rs`, `windows_invoke.rs`, `windows_bridge.rs`, `platform/portable.rs` | COM internal direction is `Variant`/`ComValue`, but legacy `RuntimeValue` conversions and portable trait APIs remain. | Keep `Variant`/`ComValue` as boundary carriers; remove `RuntimeValue` functions or move them to a named legacy module; replace portable trait value shape with `Variant`, `ComValue`, or DTO as appropriate. | `bd-pn5i.5`; Windows-only COM behavior must stay under `oxvba-com` ownership. |
| RV-11 | JIT snapshots and slot ABI compatibility | execution, public/test API, helper, tests | `crates/oxvba-jit/src/lib.rs`, `slot_abi.rs`, `jit_context.rs`, `cranelift.rs` | Normal JIT snapshots are `Variant`; test/compat APIs and RtSlot bridges still project to `RuntimeValue`. | Remove nonessential legacy snapshot methods and migrate tests to `Variant`; keep any temporary conversion only as an explicitly named compatibility boundary. | `bd-pn5i.4`. |
| RV-12 | Launcher / runner / wrapper output surfaces | wrapper/export, presentation, DTO | `crates/oxvba-launcher/src/main.rs`, host bundle snapshot APIs | Launcher prints `RuntimeValue` debug values from VM/JIT compat snapshot APIs. | Use `Variant` snapshot APIs and format through a runner result/display DTO aligned with `NATIVE_READY_RUNNER_AND_BENCHMARK_SCHEMA_V1.md`. | `bd-pn5i.6` for presentation split; phase-5 runner beads for shared schema output. |
| RV-13 | Web/language-service presentation tests | DTO/projection, tests | `crates/oxvba-web-host/src/lib.rs`, `crates/oxvba-languageservice/src/host_session.rs` | Mostly test fixtures and projections asserting host DTOs with `RuntimeValue`. | Replace with explicit web/language-service DTO expectations or host `Variant` value DTOs. | `bd-pn5i.6`. |
| RV-14 | Compiler comments and bytecode wording | stale docs/comments | `crates/oxvba-compiler/src/bytecode.rs` | Comments still describe typed instructions as returning/checking `RuntimeValue`. | Rename comments around active bytecode semantics to `Variant`/VBA value wording; no execution code dependency found in compiler. | `bd-pn5i.8` can clean stale comments during search-gate pass. |
| RV-15 | Compatibility/conformance tests | tests | `crates/*/tests/**`, large `#[cfg(test)]` modules in host/VM/JIT/HAL/COM/runtime files | Tests construct expected values with `RuntimeValue`, often because public compatibility APIs return it. | Migrate tests as each production surface migrates; where compatibility behavior itself is intentionally tested, name that boundary and keep it in the same temporary module as the compatibility API. | Same owner as touched production family; no separate blocker. |
| RV-16 | Active docs and historical evidence residues | stale-doc role, evidence, approved residual notes | `docs/ARCHITECTURE.md`, `docs/README.md`, native-ready specs/worksets, old worksets, `docs/evidence/value_model_migration/**`, `docs/evidence/v0_2/**`, `docs/IMPLEMENTATION_LOG.md` | Current native-ready docs correctly name `RuntimeValue` as residual cleanup. Older worksets/evidence/logs preserve migration history and will trip raw search gates. | Preserve provenance by archiving/demoting historical docs or recording approved residual search exceptions. Current active docs should keep only explicit residual/blocker language until the code gate is clean. | `bd-pn5i.8`; if raw docs search must become clean, historical evidence/run logs need an archive or exclusion decision before closure. |

## Downstream Bead Map

The existing delivery beads still cover the inventory, with one clarified host
observation responsibility:

- `bd-pn5i.3` (`cleanout-002`) owns RV-06, RV-07, and host-side RV-08 surfaces.
- `bd-pn5i.4` (`cleanout-003`) owns RV-11.
- `bd-pn5i.5` (`cleanout-004`) owns RV-01 through RV-05 and RV-09/RV-10.
- `bd-pn5i.6` (`cleanout-005`) owns RV-12/RV-13 and web/language-service
  consumers of host DTOs.
- `bd-pn5i.8` (`cleanout-007`) owns RV-14/RV-16 final search-gate cleanup or
  approved residual notes.

## Blocker Status

No hard blocker is known from this inventory. The main risk is scope size: the
largest families are host tests/API compatibility, HAL traits/adapters, and VM
semantic helpers. Those are already split across downstream delivery beads. If
any family cannot be fully removed in phase 2, the remaining code must be
isolated in one named compatibility module with an explicit blocker before the
search gate can close.

## Bead Self-Review / On-Track Evaluation

Review result: the inventory covers all active occurrence clusters found by the
scan and maps each cluster to an existing downstream bead. The only process
adjustment is to make host immediate/debugger/embedded observation DTOs explicit
under `cleanout-002`/`cleanout-005` rather than treating them as incidental host
tests.

On-track evaluation: on track. The scan found a large but expected migration
surface, no fake-IR code regression, and no unexpected architectural direction
change. Continue with delivery beads.
