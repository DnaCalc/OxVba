# TESTING.md

## Local lanes
- Fast lane: `./scripts/meta-check.ps1 -Fast`
- Full lane: `./scripts/meta-check.ps1`
- Matrix lane: `./scripts/meta-check.ps1 -Fast -Matrix`

## Current coverage
- Syntax: lexer/parser smoke and error tests.
- Runtime: Variant payload, coercion, arithmetic unit tests.
- IR: lowering consistency tests.
- Compiler/Host: compile+execute smoke tests.
- VM: bytecode execution test for load/add/halt flow.

## Next additions
- Property tests for parse/print and coercion matrices.
- Kani and Miri lanes on unsafe-sensitive components.
- Office harness expansion beyond MVP profile scope.
