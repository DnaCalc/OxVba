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
| `LANG-0003` | `implemented-subset` | external boundary | remaining mismatch is bounded to Excel VBIDE import-retention behavior after OxVba proves source-to-export default-member metadata preservation | none required |
| `LANG-0004` | `implemented-subset` | intentional boundary | current supported row is intentionally bounded to metadata-backed one-visible-argument default-member semantics across direct late-bound and imported early-bound invocation forms, with active compile-time diagnostics for missing/ambiguous/wrong-arity default-member cases | none required |
| `COM-0001` | `implemented-subset` | intentional boundary | current supported row is intentionally bounded to the proved scalar/object/array/BYREF late-bound subset after the review sweep reconfirmed the active registered CreateObject success/repetition, error-shape, and `OxVba.TestEventServer` scalar/object/array/array-return marshaling lanes | none required |
| `COM-0003` | `verified` | remaining accepted scope | broader imported typelib behavior remains open under the current supported subset boundary after the registered `OxVba.TestEventServer` Ping parity slice and active `WithEvents` callback regressions | `none required` |
| `COM-0004` | `verified` | intentional boundary | current supported row is intentionally bounded to the reviewed registered-host `Scripting.Dictionary` and `Scripting.FileSystemObject` subsets plus the active loaded `.basproj` ordering and broken-first/later-valid tolerance family for the file-backed TestEventServer lane | none required |
| `COM-0005` | `verified` | intentional boundary | current supported row is intentionally bounded to the reviewed mixed-server dispatch/vtable strategy subset | none required |
| `COM-0006` | `verified` | intentional boundary | current supported row is intentionally bounded to the file-backed TestEventServer family, including loaded `.basproj` ordering, unresolved-diagnostic, and broken-first/later-valid tolerance coverage | none required |
| `PH-0001` | `implemented-subset` | intentional boundary | startup-object/forms/designer-backed startup intentionally excluded from current strict lane | none required |
| `PH-0002` | `implemented-subset` | intentional boundary | host-observable top-level basproj/VBP mainline, mixed-declaration preservation, and project-hosted helper-procedure shared module-state lanes are now proved; remaining exclusions stay bounded to the documented program/script-only subset boundary | none required |
| `PH-0003` | `implemented-subset` | intentional boundary | current supported discovery lane is explicitly bounded to strict deterministic discovery | none required |
| `PH-0004` | `implemented-subset` | intentional boundary | current VBP-S0 subset intentionally excludes designer/startup-object surfaces | none required |
| `PH-0005` | `implemented-subset` | intentional boundary | external COM typelib resolution is owned in the COM matrix, not here | none required |
| `PH-0006` | `implemented-subset` | intentional boundary | current supported row is intentionally bounded to host-module scaffolds and extension-module project shape; deeper host lifecycle and Office-style host-project parity are outside this row's accepted subset | none required |
| `PH-0007` | `implemented-subset` | external boundary | residual mismatch is bounded to Excel VBIDE import-retention behavior | none required |
| `PH-0008` | `implemented-subset` | external boundary | imported NewEnum runtime mismatch is now bounded to Excel VBIDE import-retention behavior after LANG-0002 closes the accepted OxVba execution surface | none required |
| `PH-0009` | `implemented-subset` | intentional boundary | current supported row is intentionally bounded to deterministic policy-denial plus Windows host-backed `interactive_dev` `Shell`/`Dir`/`Environ` behavior; broader host-project lifecycle stays outside this row | none required |
| `PH-0010` | `in-progress` | external boundary | supported `.basproj` / VBP adapter roundtrip is locally evidenced and the current oracle surface is registered, but broader MS-OVBA parity is blocked by the external Foundation extraction-depth gap tracked by `ODG-042` | none required |
| `LSF-0001` | `in-progress` | intentional boundary | bounded internal language-service surface is green (syntax tree, semantic snapshot, workspace invalidation, diagnostics, symbols, completions, signature help, go-to-definition, references, hover); this row intentionally records the current internal service-surface inventory rather than full LSP parity or feature-complete language-service coverage | none required |
| `LSF-0002` | `in-progress` | intentional boundary | current formal language/compiler representation remains intentionally bounded to the Lean scaffold plus obligation/deferred-gate registry, without claiming proof closure or full representation coverage | none required |

## 3. Notes

1. This register is not a second feature matrix.
2. It exists to ensure bounded-slice honesty does not drain the tracker while accepted remaining work still exists.
3. Rows marked `intentional boundary` or `external boundary` do not require new OxVba delivery beads unless project scope changes.
4. Rows marked `remaining accepted scope` require active tracker ownership and an open bead path.
5. During review-heavy phases, one active residual review sweep bead may intentionally own multiple rows in the same major domain to keep execution focused on meaningful batched work rather than micro-successor churn.
