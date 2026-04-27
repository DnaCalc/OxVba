# V0.2 Late-Bound IDispatch Supported Matrix

Date: 2026-04-27
Owner: Codex
Bead: `bd-bqm8.3.2`
Status: complete

## Supported Rows

| Row | Behavior | V0.2 status | Required evidence |
| --- | --- | --- | --- |
| LBD-001 | ProgID activation for controlled and registered Windows COM lanes | supported | host COM activation tests and conformance notes |
| LBD-002 | Token-backed dispatch invocation through the deterministic projection floor | supported | HAL/host COM invocation tests |
| LBD-003 | Name-backed member resolution when authoritative metadata maps the name to a DISPID/member spec | supported | compiler/host metadata-resolution tests |
| LBD-004 | Positional method/property-get invocation for scalar and object results | supported | controlled COM VM/JIT/host parity tests |
| LBD-005 | Positional method/property-get invocation for string, array, and wide numeric payloads | supported | controlled COM value-shape tests |
| LBD-006 | Named arguments when authoritative metadata supplies DISPIDs | supported | late-bound named-argument host tests |
| LBD-007 | Default-member dispatch when imported/typelib metadata supplies default-member identity | supported | default-member dispatch tests |
| LBD-008 | Event callback argument payload projection for controlled connection-point lanes | supported | HAL COM event callback tests |
| LBD-009 | Deterministic diagnostics for unsupported, ambiguous, or metadata-missing late-bound shapes | supported | diagnostic taxonomy and negative tests |

## Explicit Unsupported Rows

| Row | Behavior | V0.2 status | Reason |
| --- | --- | --- | --- |
| LBD-U01 | Full Office-wide behavioral parity for arbitrary `IDispatch` servers | unsupported | requires a broader Office/Excel/Access corpus beyond this lane |
| LBD-U02 | Natural untyped default-member syntax without authoritative metadata | unsupported | would guess default-member identity and produce fuzzy parity |
| LBD-U03 | Arbitrary optional-argument/missing-argument synthesis without metadata | unsupported | missing-argument shape is server/member-specific |
| LBD-U04 | General property-put/property-set parity beyond fixture-proved rows | unsupported | requires expanded setter corpus and error mapping |
| LBD-U05 | Non-Windows COM late-bound parity | unsupported | V0.2 COM lane is Windows-primary |

## Closure Rule

`bd-bqm8.3` can close only when supported rows have executable evidence and
the unsupported rows above remain explicit in docs/conformance outputs.

This matrix is intentionally bounded: it prevents unsupported late-bound COM
behavior from being treated as implemented by architecture prose alone.

## Verification

Passed:

- `rg -n "late-bound|IDispatch|default-member|named argument|missing" docs/spec docs/evidence docs/worksets -g "*.md"`
