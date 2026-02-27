# TESTING.md

## Local lanes
- Fast lane: `./scripts/meta-check.ps1 -Fast`
- Full lane: `./scripts/meta-check.ps1`
- Matrix lane: `./scripts/meta-check.ps1 -Fast -Matrix`
- Formal lane (non-blocking): `./scripts/meta-check.ps1 -Fast -Formal`
- Combined ladder lane: `./scripts/meta-check.ps1 -Fast -Conformance -Matrix -Formal`

## Current coverage
- Syntax: lexer/parser smoke and error tests.
- Runtime: Variant payload, coercion, arithmetic unit tests.
- IR: lowering consistency tests.
- Compiler/Host: compile+execute smoke + control-flow compilation tests.
- VM: bytecode execution tests for arithmetic, branch/loop execution, and jump validation.
- Conformance: VM/JIT-toggle profile corpus with slot snapshots (including relational/boolean branches).
- Formal: profile-scoped non-blocking obligations via `scripts/run-formal.ps1`.

## Next additions
- Property tests for parse/print and coercion matrices.
- Broader Kani lane coverage once tooling is available in CI/local environments.
- Office harness expansion beyond MVP profile scope.
