# TESTING.md

## Local lanes
- Fast lane: `./scripts/meta-check.ps1 -Fast`
- Full lane: `./scripts/meta-check.ps1`

## Current coverage
- Syntax: lexer/parser smoke and error tests.
- Runtime: Variant payload, coercion, arithmetic unit tests.
- IR: lowering consistency tests.
- Compiler/Host: compile+execute smoke tests.
- VM: bytecode execution test for load/add/halt flow.

## Next additions
- Conformance corpus execution with golden files.
- Property tests for parse/print and coercion matrices.
- Kani and Miri lanes on unsafe-sensitive components.
