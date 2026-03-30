# Validation Residual Scope Register

Date: 2026-03-30  
Status: active  
Purpose: make the remaining accepted work outside bounded-slice validation rows explicit, and tie that residual work to active execution owners and bead paths.

## 1. Rule

For canonical matrix rows with truth states such as `implemented-subset`, `in-progress`, or `planned`, the repo must distinguish between:
1. intentional boundary for the currently accepted scope,
2. external boundary outside OxVba execution ownership,
3. remaining accepted scope that still requires OxVba delivery work.

Only the third category requires active open work in the tracker.

## 2. Residual Register

| Row | Current truth | Residual disposition | Residual summary | Execution owner |
|---|---|---|---|---|
| `LANG-0001` | `implemented-subset` | remaining accepted scope | broader `For Each` semantics still require object-enumerator completion | `bd-cyr.2.7` via `LANG-0002` |
| `LANG-0002` | `in-progress` | remaining accepted scope | object-enumerator `For Each` breadth remains open beyond the currently proved project-dynamic/imported-collection direct/callable/bundle session slices, imported-COM transport, registered `OxVba.TestDispatch` transport/direct-session VM/JIT parity, and loaded Excel-imported direct/bundle session VM/JIT slices | `bd-cyr.2.17` |
| `LANG-0003` | `implemented-subset` | external boundary | remaining mismatch is bounded to Excel VBIDE import-retention behavior after OxVba proves source-to-export default-member metadata preservation | none required |
| `LANG-0004` | `implemented-subset` | intentional boundary | current supported row is intentionally bounded to metadata-backed one-visible-argument default-member semantics across direct late-bound and imported early-bound invocation forms, with active compile-time diagnostics for missing/ambiguous/wrong-arity default-member cases | none required |
| `COM-0001` | `implemented-subset` | remaining accepted scope | broader late-bound COM invocation and marshalling behavior remains outside the proved scalar/object/array subset, after landing the wide-runtime `I64 -> VT_I8` scalar and variant-array normalization slice plus the active registered CreateObject success/repetition, error-shape, and `OxVba.TestEventServer` scalar/object/array/array-return marshaling lanes | `bd-cyr.3.14` |
| `COM-0003` | `verified` | remaining accepted scope | broader imported typelib behavior remains open under the current supported subset boundary after the registered `OxVba.TestEventServer` Ping parity slice and active `WithEvents` callback regressions | `none required` |
| `COM-0004` | `verified` | remaining accepted scope | broader real-library activation authority remains open beyond the current registered-host subset and newly active loaded `.basproj` ordering plus broken-first/later-valid tolerance subset | `bd-cyr.3.14` |
| `COM-0005` | `verified` | remaining accepted scope | broader dual-interface behavior remains open outside the mixed-server supported subset | `bd-cyr.3.14` |
| `COM-0006` | `verified` | intentional boundary | current supported row is intentionally bounded to the file-backed TestEventServer family, including loaded `.basproj` ordering, unresolved-diagnostic, and broken-first/later-valid tolerance coverage | none required |
| `PH-0001` | `implemented-subset` | intentional boundary | startup-object/forms/designer-backed startup intentionally excluded from current strict lane | none required |
| `PH-0002` | `implemented-subset` | remaining accepted scope | deeper mixed-source and module-state semantics remain open | `bd-cyr.4.2` |
| `PH-0003` | `implemented-subset` | intentional boundary | current supported discovery lane is explicitly bounded to strict deterministic discovery | none required |
| `PH-0004` | `implemented-subset` | intentional boundary | current VBP-S0 subset intentionally excludes designer/startup-object surfaces | none required |
| `PH-0005` | `implemented-subset` | intentional boundary | external COM typelib resolution is owned in the COM matrix, not here | none required |
| `PH-0006` | `implemented-subset` | remaining accepted scope | deeper host lifecycle and Office-style host-project parity remain open | `bd-cyr.4.2` |
| `PH-0007` | `implemented-subset` | external boundary | residual mismatch is bounded to Excel VBIDE import-retention behavior | none required |
| `PH-0008` | `in-progress` | remaining accepted scope | imported NewEnum runtime behavior remains open where it depends on broader language/project parity | `bd-cyr.4.2` |
| `PH-0009` | `implemented-subset` | remaining accepted scope | broader host-project lifecycle and execution-environment breadth remain open | `bd-cyr.4.2` |
| `PH-0010` | `in-progress` | remaining accepted scope | MS-OVBA storage roundtrip/oracle depth remains open | `bd-cyr.4.2` |
| `LSF-0001` | `in-progress` | remaining accepted scope | language-service feature coverage beyond current syntax/semantic surface remains open | `bd-cyr.5.1` |
| `LSF-0002` | `in-progress` | remaining accepted scope | formal language/compiler representation remains partial | `bd-cyr.5.1` |

## 3. Notes

1. This register is not a second feature matrix.
2. It exists to ensure bounded-slice honesty does not drain the tracker while accepted remaining work still exists.
3. Rows marked `intentional boundary` or `external boundary` do not require new OxVba delivery beads unless project scope changes.
4. Rows marked `remaining accepted scope` require active tracker ownership and an open bead path.
5. During review-heavy phases, one active residual review sweep bead may intentionally own multiple rows in the same major domain to keep execution focused on meaningful batched work rather than micro-successor churn.
