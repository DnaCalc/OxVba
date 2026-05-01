# RuntimeValue And IR Stub Cleanout Workset

Status: `in-progress`
Date: 2026-04-30
Parent: `WORKSET_2026-04-30_NATIVE_READY_REBASE_MASTER.md`

## Purpose

Remove stale implementation surfaces that obscure the native-ready architecture:
the legacy `RuntimeValue` carrier and the fake HIR/MIR/CFG scaffold.

Initial 2026-04-30 state: the fake HIR/MIR/CFG scaffold and compiler
`lower_to_hir` no-op have been removed from active code. `RuntimeValue` remains
a broad migration spanning runtime, HAL, COM, host, JIT, launcher, web, and
tests.

## Scope

In scope:

- Audit every active `RuntimeValue` use in crates and non-archived docs.
- Replace active runtime, host, JIT, wrapper, web, launcher, and language-service
  APIs with retained `Variant` or explicit presentation DTOs.
- Remove `RuntimeValue` from public re-exports once replacements exist.
- Delete or quarantine `oxvba-ir` HIR/MIR/CFG stubs and `lower_to_hir` if no
  real pipeline consumes them. This slice was completed on 2026-04-30.
- Remove stale dependency edges that exist only for scaffold architecture.

Out of scope:

- Replacing the current bytecode compiler.
- Introducing a real native compiler IR before the value/type cleanup is stable.
- Preserving old compatibility APIs without a named blocker.

## Execution Epics

1. **RuntimeValue Inventory**
   - Classify uses as execution, helper, public API, tests, DTO/projection,
     wrapper/export, or stale docs.
   - Close condition: inventory identifies deletion path or blocker for every
     active use family.
2. **Variant API Migration**
   - Replace execution-facing APIs with `Variant` and update callers.
   - Close condition: VM/JIT/host/wrapper paths do not require `RuntimeValue`.
3. **Presentation DTO Split**
   - Create display/projection DTOs only where UI/web/language-service callers
     need stable JSON-like shapes.
   - Close condition: no UI surface uses semantic runtime carriers as DTOs.
4. **IR Scaffold Removal**
   - Remove `CfgIr`, `VbaHir`, `VbaMir`, no-op optimizers, and unused lowerers,
     or quarantine behind an explicit migration note.
   - Close condition: fake IR names do not appear in active crate APIs.
   - 2026-04-30 state: completed for active code.
5. **Search Gate**
   - Close condition: search gates are clean outside approved residual notes.

## Current RuntimeValue Inventory Snapshot

Detailed evidence is recorded in
[`../evidence/native_ready/RUNTIMEVALUE_ACTIVE_USE_INVENTORY_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_ACTIVE_USE_INVENTORY_2026-05-01.md).

The 2026-05-01 scan found `RuntimeValue` in 63 crate files / 3066 crate
occurrences and 270 non-archived doc files / 1068 non-archived doc occurrences
(excluding `docs/archive/**` and `docs/spec/archive/**`). Active use families
are not a single-file rename; they are grouped as:

- runtime enum/re-export and `Variant` bridge helpers;
- `SafeArray`, pointer-helper, string/coercion, and runtime compatibility APIs;
- VM slots/snapshots/invocation plus legacy semantic helper families;
- host engine/session/event surfaces plus immediate/debugger/embedded value DTOs;
- HAL trait/adapters and recording/replay compatibility surfaces;
- COM model, dynamic object, Windows bridge/invoke/variant conversion, and
  portable COM surfaces;
- JIT snapshot/slot ABI compatibility surfaces;
- launcher/web/language-service presentation surfaces;
- tests, stale compiler comments, and historical non-archived evidence/docs.

The migration must proceed by boundary family, not by global search/replace.
If any residual survives the delivery beads, it must be isolated in one named
compatibility module or approved residual note before the search gate can close.

## First Beads

- `cleanout-000`: roll out the phase-2 executable bead path. Done 2026-05-01.
- `cleanout-001`: produce `RuntimeValue` active-use inventory. Done 2026-05-01.
- `cleanout-002`: migrate VM and host snapshot/invoke/observation surfaces,
  including immediate/debugger/embedded host-side value DTOs.
- `cleanout-003`: migrate JIT compatibility snapshot and slot ABI surfaces.
- `cleanout-004`: migrate HAL, COM, runtime helper, and compatibility adapter
  boundaries.
- `cleanout-005`: split launcher/web/language-service presentation DTOs.
- `cleanout-006`: verify completed IR scaffold removal. Done 2026-05-01.
- `cleanout-007`: run `RuntimeValue` and fake IR search gates and document any
  approved residuals or blockers.

## Terminal Gate

This workset is complete when:

- `RuntimeValue` is gone from active code, or any residual is isolated in one
  named blocker module with a removal date.
- Fake IR scaffold is gone from active crate APIs.
- `cargo test --workspace` is green, excluding documented environment-specific
  lanes.
- The native-ready specs and docs no longer need compatibility caveats for these
  surfaces.
