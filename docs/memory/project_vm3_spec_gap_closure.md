# VM3 Spec Gap Closure Memory

## 2026-07-01 - `currency-mul-f64-lossy` (`bd-4ktq.8`)

- Closed the Currency arithmetic value-typing gap by adding an exact scaled
  `i128` lane in `crates/oxvba-eval/src/arith.rs` for Currency `+`, `-`, and
  `*`.
- The lane applies to `Checked(Currency)` typed arithmetic and to Variant
  widening when a Currency operand combines with exact integer-compatible
  operands. Non-exact operands still use the existing coercion path.
- Multiplication divides by the Currency scale (`10_000`) with half-scaled-unit
  ties-to-even rounding, preserves Currency subtype in vm3, and raises Overflow
  (6) at the scaled `i64` boundary.
- Currency-to-Currency and exact integer-compatible Currency coercion now stays
  on the exact path, avoiding f64 re-rounding near the boundary.
- Differential coverage lives in
  `crates/oxvba-differential/tests/currency_arithmetic_vm3.rs`.
- Verification passed:
  - `cargo test -p oxvba-eval currency`
  - `cargo test -p oxvba-differential --test currency_arithmetic_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
  - `cargo clippy --workspace --all-targets` exited 0; warn-level findings
    remained in unrelated crates/tests.
- Live Excel retry note: the first probe produced a VBA compile modal
  (`Expected array`) because helper function `D` was shadowed by local Currency
  variable `d`. UI Automation captured selected token `d` and the line
  `"mul_near=" & d(a * b) & vbLf & _`; the owned PID-scoped dialog/process was
  dismissed and stopped.
- New standing oracle rule recorded in `AGENTS.md`, `docs/CONFORMANCE.md`, and
  `docs/memory/EXCEL_VBA_ORACLE_MODAL_HANDLING.md`: always prepare a
  PID-scoped UI Automation watcher/helper for Excel/VBA compile/runtime modals,
  and treat `Application.Run` macro-availability errors as ambiguous until a
  VBE Debug -> Compile diagnostic is captured.

## 2026-07-01 - Scoping Visibility Fixture Baseline (`bd-4ktq.9.1`)

- Created the fixture-first truth surface for the multi-module scoping batch
  under `bd-4ktq.9`.
- Live Excel/VBA oracle evidence lives in
  `docs/evidence/conformance/vm3_scoping_visibility_oracle_20260701T0945Z/`.
  The runner invokes VBE Debug -> Compile VBAProject through command id `578`,
  captures owned compile modals with UI Automation, and kills only the owned
  Excel PID for each case.
- Oracle matrix:
  - same-module `Private` function: compiles and runs (`7`),
  - cross-module unqualified `Private`: `Sub or Function not defined`,
  - cross-module `Module.PrivateMember`: `Method or data member not found`,
  - duplicate Public unqualified member: `Ambiguous name detected: Dup`,
  - module-name/Public-member collision: `Expected variable or procedure, not module`,
  - valid `VBAProject.Module.Member`: compiles and runs (`13`),
  - wrong project qualifier under `Option Explicit`: `Variable not defined`,
  - `Friend` in a standard module: `Only valid in object module`,
  - `Friend` in a class module: compiles and runs (`19`).
- Added `crates/oxvba-differential/tests/scoping_visibility_vm3.rs`.
  Current-green tests cover the legal baseline shapes; ignored tests encode the
  oracle-backed expected failures for `bd-4ktq.9.2` through `bd-4ktq.9.6`.
- Verification passed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`

## 2026-07-01 - Private Module Visibility (`bd-4ktq.9.2`)

- Closed `intra-project-private-not-enforced` for same-project standard-module
  leakage.
- `ProjectProvider::MemberEntry` now carries scanner-owned `Visibility`.
  Project-level unqualified lookup only publishes `Public` members, and
  `Module.Member` / `Project.Module.Member` qualified lookup uses a public-only
  owner-member resolver. The existing all-member owner resolver remains for
  typed member paths so class/internal member mechanics are not broadly
  rewritten in this bead.
- Same-module `Private` access remains valid through the source scope chain,
  which is consulted before provider lookup.
- Flipped on the oracle-backed scoping fixture assertions for:
  - cross-module unqualified `Private` -> rejected,
  - cross-module `Module.PrivateMember` -> rejected.
- Verification passed:
  - `cargo test -p oxvba-differential --test scoping_visibility_vm3`
  - `cargo test -p oxvba-symbol`
  - `cargo test -p oxvba-bind`
  - `cargo test -p oxvba-differential --lib vm3_golden_snapshot`
