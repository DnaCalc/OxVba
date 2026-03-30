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
| `LANG-0001` | `implemented-subset` | remaining accepted scope | broader `For Each` semantics still require object-enumerator completion | `bd-cyr.2.5` via `LANG-0002` |
| `LANG-0002` | `in-progress` | remaining accepted scope | object-enumerator `For Each` breadth remains open beyond the currently proved project-dynamic/imported-collection, imported-COM, and direct session-invocation slices | `bd-cyr.2.5` |
| `LANG-0003` | `implemented-subset` | external boundary | remaining mismatch is bounded to Excel VBIDE import-retention behavior after OxVba proves source-to-export default-member metadata preservation | none required |
| `LANG-0004` | `implemented-subset` | remaining accepted scope | broader late-bound default-member dispatch breadth remains open beyond the current one-visible-argument named/positional and runtime-string selector subset | `bd-cyr.2.6` |
| `COM-0001` | `implemented-subset` | remaining accepted scope | broader late-bound COM invocation and marshalling behavior remains outside the proved scalar/object/array subset, after landing the wide-runtime `I64 -> VT_I8` scalar and variant-array normalization slice | `bd-cyr.3.4` |
| `COM-0003` | `verified` | remaining accepted scope | broader imported typelib behavior remains open under the current supported subset boundary | `bd-cyr.3.5` |
| `COM-0004` | `verified` | remaining accepted scope | broader real-library activation authority remains open beyond the current registered-host subset | `bd-cyr.3.5` |
| `COM-0005` | `verified` | remaining accepted scope | broader dual-interface behavior remains open outside the mixed-server supported subset | `bd-cyr.3.5` |
| `COM-0006` | `verified` | remaining accepted scope | broader typelib/versioned-reference behavior remains open outside the current file-backed subset | `bd-cyr.3.5` |
| `PH-0001` | `implemented-subset` | intentional boundary | startup-object/forms/designer-backed startup intentionally excluded from current strict lane | none required |
| `PH-0002` | `implemented-subset` | remaining accepted scope | deeper mixed-source and module-state semantics remain open | `bd-cyr.4.1` |
| `PH-0003` | `implemented-subset` | intentional boundary | current supported discovery lane is explicitly bounded to strict deterministic discovery | none required |
| `PH-0004` | `implemented-subset` | intentional boundary | current VBP-S0 subset intentionally excludes designer/startup-object surfaces | none required |
| `PH-0005` | `implemented-subset` | intentional boundary | external COM typelib resolution is owned in the COM matrix, not here | none required |
| `PH-0006` | `implemented-subset` | remaining accepted scope | deeper host lifecycle and Office-style host-project parity remain open | `bd-cyr.4.2` |
| `PH-0007` | `implemented-subset` | external boundary | residual mismatch is bounded to Excel VBIDE import-retention behavior | none required |
| `PH-0008` | `in-progress` | remaining accepted scope | imported NewEnum runtime behavior remains open where it depends on broader language/project parity | `bd-cyr.4.2` |
| `PH-0009` | `implemented-subset` | remaining accepted scope | broader host-project lifecycle and execution-environment breadth remain open | `bd-cyr.4.2` |
| `PH-0010` | `in-progress` | remaining accepted scope | MS-OVBA storage roundtrip/oracle depth remains open | `bd-cyr.4.3` |
| `LSF-0001` | `in-progress` | remaining accepted scope | language-service feature coverage beyond current syntax/semantic surface remains open | `bd-cyr.5.1` |
| `LSF-0002` | `in-progress` | remaining accepted scope | formal language/compiler representation remains partial | `bd-cyr.5.2` |

## 3. Notes

1. This register is not a second feature matrix.
2. It exists to ensure bounded-slice honesty does not drain the tracker while accepted remaining work still exists.
3. Rows marked `intentional boundary` or `external boundary` do not require new OxVba delivery beads unless project scope changes.
4. Rows marked `remaining accepted scope` require active tracker ownership and an open bead path.
