# WORKSET_2026-02-27_CONTROLFLOW_V2.md

## Purpose
Define the next execution-grade work set after Phase 12 `mvp-int32-core-v1` stabilization, with enough detail for direct parallel agent implementation.

Related long-horizon roadmap:
- `docs/worksets/PROFILE_LADDER_2026-02-27_MACH1000.md` (v2-v21 profile queue)

## Program Name
`WS-CF-V2` (Control-Flow and Semantics Expansion)

## Strategic Outcome
Expand OxVBA from arithmetic-only MVP into a small structured-control-flow language slice while preserving the current green gate discipline.

## Exit Gate
All items below are required for completion.

1. New profile scope `mvp-controlflow-v2` declared in docs.
2. Conformance corpus includes passing `If` and `For` cases.
3. Divergences `DIV-0001` and `DIV-0002` closed or replaced with narrower residual divergences.
4. Matrix gate green for required cells in the declared scope.
5. `./scripts/meta-check.ps1 -Fast -Conformance -Matrix` green on local Windows.
6. CI includes matrix gate for this new scope.
7. Formal obligations for `v2` are executed and recorded (non-blocking policy):
   - Kani harness for jump-target bounds / `pc` progression.
   - Kani harness for temp-slot allocator non-overlap with declared slots.
   - Unresolved tooling/failures are tracked in formal extended todo evidence.

## Scope
Implement these semantics end-to-end.

1. `If ... Then ... End If` with integer equality condition.
2. `For i = a To b ... Next i` with ascending step `+1` only.
3. Integer expressions in assignment RHS:
   - constant (`x = 10`)
   - self-add/sub constant (`x = x + 5`, `x = x - 3`)
   - variable copy (`x = y`)
4. Continue supporting `Option Explicit`.
5. Keep current VM-first execution; JIT remains optional/scaffold unless explicitly implemented in this work set.

## Out Of Scope
1. `Else`, `ElseIf`, `Step`, descending loops.
2. General expression parser with precedence beyond current subset.
3. COM object model expansion.
4. Real Cranelift JIT generation.

## Architecture Decision For This Work Set
Introduce a tiny structured bound model in compiler layer and compile it to jump-capable bytecode.

### Decision
Use structured `BoundStmt`/`BoundExpr` in `oxvba-compiler` instead of extending line-by-line `BoundOp` heuristics further.

### Why
1. `If` and `For` require block scoping and jump target patching.
2. It creates a direct path to later HIR lowering.
3. It avoids fragile string parsing in emission stage.

## Detailed Implementation Plan

### Track A: Compiler Front-End Refactor (`oxvba-compiler`)

Files:
- `crates/oxvba-compiler/src/resolve.rs`
- `crates/oxvba-compiler/src/typecheck.rs`
- `crates/oxvba-compiler/src/lib.rs`

Tasks:
1. Replace `BoundOp` with structured nodes:
   - `BoundModule { option_explicit, declarations, body: Vec<BoundStmt> }`
   - `BoundStmt::Assign { target, expr }`
   - `BoundStmt::IfEq { lhs: BoundExpr, rhs: BoundExpr, then_body: Vec<BoundStmt> }`
   - `BoundStmt::ForRange { var, start: BoundExpr, end: BoundExpr, body: Vec<BoundStmt> }`
2. Add minimal `BoundExpr`:
   - `IntConst(i32)`
   - `Var(String)`
   - `AddConst { var, delta }`
   - `SubConst { var, delta }`
3. Implement block parsing using stack frames from source lines:
   - detect `If ... Then` open, `End If` close
   - detect `For ... To ...` open, `Next` close
4. Preserve current behavior for unsupported syntax by emitting explicit `Unsupported` diagnostics with original line text.
5. Extend typecheck:
   - declaration checks on all variable references
   - loop variable declaration behavior under `Option Explicit`
   - assignment target and expression variable validation

Acceptance:
1. Unit tests for parser-to-bound conversion for `If` and `For` blocks.
2. Unit tests for undeclared variables in nested blocks.

### Track B: Bytecode And Emitter (`oxvba-compiler`)

Files:
- `crates/oxvba-compiler/src/bytecode.rs`
- `crates/oxvba-compiler/src/emit.rs`

Tasks:
1. Add jump-capable instructions:
   - `CopySlot { dst, src }`
   - `CmpEqSlots { dst, lhs, rhs }` (stores `1` or `0` in `dst`)
   - `JumpIfZero { cond_slot, target_pc }`
   - `Jump { target_pc }`
   - `IncSlot { slot }`
   - `CmpLeSlots { dst, lhs, rhs }`
2. Introduce temporary slot allocator in emitter for condition/loop temporaries.
3. Implement backpatching for jump targets.
4. Emit `ForRange` lowering pattern:
   - init loop var
   - loop head compare (`var <= end`)
   - conditional jump to exit
   - body
   - increment var
   - jump to head
5. Preserve serialization derivations and tests for bytecode enums.

Acceptance:
1. Emitter tests assert opcode sequences for representative `If` and `For` programs.
2. Slot count includes declared + temporary slots deterministically.

### Track C: VM Interpreter Upgrade (`oxvba-vm`)

Files:
- `crates/oxvba-vm/src/interpreter.rs`
- `crates/oxvba-vm/src/lib.rs`

Tasks:
1. Replace `for instr in ...` execution with `pc`-driven loop.
2. Implement handlers for new control-flow opcodes.
3. Add bounds checks for jump targets and slot indices.
4. Preserve existing behavior for arithmetic ops and halt.

Acceptance:
1. VM unit tests for branch taken/not-taken paths.
2. VM unit tests for loop execution and final slot values.
3. VM rejects invalid jump targets with descriptive error.

### Track D: Host/CLI Behavior (`oxvba-host`, `oxvba-cli`)

Files:
- `crates/oxvba-host/src/engine.rs`
- `crates/oxvba-cli/src/main.rs`

Tasks:
1. Keep CLI surface stable (`run <file> --dump-slots [--jit]`).
2. Add optional `--trace-bytecode` output mode for orientation and debugging.
3. Keep `--jit` semantics explicit in docs: toggle path enabled, VM fallback active.

Acceptance:
1. CLI tests for argument combinations and unknown flags.
2. Snapshot output unchanged unless trace flag used.

### Track E: Conformance Expansion (`conformance/` + scripts)

Files:
- `conformance/tests/*.bas`
- `conformance/golden/smoke.csv`
- `scripts/run-conformance.ps1`
- `scripts/run-matrix.ps1`

Tasks:
1. Add tests:
   - `if_true.bas`
   - `if_false.bas`
   - `for_basic.bas`
   - `for_zero_iter.bas`
   - `nested_if_for.bas`
2. Capture/update expected `slots` in golden CSV.
3. Ensure backend parameterization remains valid (`vm`, `jit`).
4. Expand matrix output columns if needed for new profile id.

Acceptance:
1. Conformance run passes on all tests in both backend modes.
2. Matrix gate report shows required cells green for new scope.

### Track F: Divergence Lifecycle

Files:
- `docs/evidence/divergences/DIV-0001.md`
- `docs/evidence/divergences/DIV-0002.md`
- `docs/evidence/divergences/README.md`

Tasks:
1. Re-run divergence fixtures after implementing `If` and `For`.
2. If passing, mark records closed with closure date and commit id.
3. If partially unresolved, split into narrower divergence IDs and archive old IDs with supersession notes.

Acceptance:
1. Divergence index reflects current truth with reproducible commands.

### Track G: Documentation And Status

Files:
- `docs/CONFORMANCE.md`
- `docs/TESTING.md`
- `docs/BYTECODE_FORMAT.md`
- `docs/VM_ARCHITECTURE.md`
- `docs/IMPLEMENTATION_LOG.md`
- `docs/PHASE12_STATUS.md` or successor status file
- `MACH1000_PLAN.md` (status annotation only)

Tasks:
1. Declare new profile scope `mvp-controlflow-v2`.
2. Document new opcode set and control-flow semantics.
3. Update implementation log with concrete additions and gates.
4. Record exact gate artifact paths.

Acceptance:
1. Docs and evidence are internally consistent.
2. No stale references to previous profile as current target.

### Track H: CI And Automation

Files:
- `.github/workflows/ci.yml`
- `scripts/meta-check.ps1`

Tasks:
1. Keep matrix gate invocation in CI for Windows lane.
2. Optionally add Linux matrix lane to verify parser/compiler/vm parity where COM is not required.
3. Fail CI on missing matrix artifact generation.
4. Add formal-lane invocation for `v2` Kani obligations.

Acceptance:
1. CI green with updated corpus.
2. Matrix artifacts regenerated on local runs and CI logs.
3. Formal lane artifacts are reproducible and green.

## Parallelization Plan (Agent-Coding Optimized)

Run these tracks in parallel groups.

Group 1 (can start immediately):
1. Track A (compiler front-end refactor).
2. Track B (bytecode/emitter design and tests).
3. Track E (conformance fixture authoring and golden expectations draft).

Group 2 (start when B has opcode contract):
1. Track C (VM pc-loop and opcode handlers).
2. Track D (CLI trace output + argument tests).

Group 3 (start after C and E pass locally):
1. Track F (divergence closure updates).
2. Track G (docs/status updates).
3. Track H (CI/automation adjustments).

## Suggested Commit Slices

1. `compiler: introduce structured bound model for if/for`
2. `compiler: add jump-capable bytecode and emitter backpatching`
3. `vm: pc-based interpreter with branch/loop opcodes`
4. `conformance: add control-flow corpus and golden updates`
5. `docs: close divergences and declare mvp-controlflow-v2 scope`
6. `ci/scripts: enforce new matrix gate path`

## Verification Checklist

Run in this order.

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `./scripts/run-conformance.ps1 -Backend vm`
5. `./scripts/run-conformance.ps1 -Backend jit`
6. `./scripts/run-matrix.ps1`
7. `./scripts/meta-check.ps1 -Fast -Conformance -Matrix`

## Risk Register For This Work Set

1. Parser complexity jumps too fast.
   - Mitigation: strict subset grammar, explicit unsupported diagnostics.
2. Emitter/control-flow bugs from incorrect jump patching.
   - Mitigation: opcode sequence unit tests and PC trace mode.
3. Slot allocator collisions between user slots and temporaries.
   - Mitigation: separate temp slot range after declared slots.
4. JIT toggle confusion (users expecting real JIT execution).
   - Mitigation: explicit docs + optional runtime notice when JIT compile is not enabled.

## Definition Of Done For WS-CF-V2

1. All exit gate items green.
2. Divergence docs reflect post-implementation reality.
3. New status tour generated in `docs/status-tours/` demonstrating `If` and `For` end-to-end.
4. Changes committed and pushed with reproducible evidence artifacts.

## Immediate Next Command Sequence

1. Create feature branch or continue on `master` per repository convention.
2. Execute Group 1 tracks in parallel.
3. Merge Group 1 outputs and lock opcode contract.
4. Execute Group 2 tracks.
5. Run verification checklist.
6. Execute Group 3 for docs/evidence/CI finalization.
7. Commit, push, and regenerate status tour.
