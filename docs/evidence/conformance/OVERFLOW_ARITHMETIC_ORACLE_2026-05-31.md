# Arithmetic Overflow Oracle (Excel) — bd-0d1y

Captured from Excel 16 (`AccessVBOM=1`) on 2026-05-31 via a single-session VBA probe
(`scripts`-style COM injection: add module → `AddFromString` → `Application.Run`, each case
under `On Error Resume Next`, reporting `Err.Number` or `TypeName(r) & ":" & CStr(r)`).

This is the ground-truth spec target for OxVba arithmetic overflow (bead `bd-0d1y`).

## Probe results

| Case | VBA source (abbrev.) | Excel result |
|---|---|---|
| INT_ADD_ASSIGN | `Dim ai As Integer: ai=32767: ai=ai+1` | `ERR6` |
| INT_ADD_EXPR | `ai=32767: r=ai+1` (r Variant) | `ERR6` |
| INT_NOOVF | `ai=100: r=ai+1` | `Integer:101` |
| INT_SUB_ASSIGN | `ai=-32768: ai=ai-1` | `ERR6` |
| INT_MUL_EXPR | `ai=1000: r=ai*1000` | `ERR6` |
| INT_NEG_EXPR | `ai=-32768: r=-ai` | `ERR6` |
| LNG_ADD_ASSIGN | `Dim al As Long: al=2000000000: al=al+2000000000` | `ERR6` |
| LNG_ADD_EXPR | `al=2000000000: r=al+al` | `ERR6` |
| LNG_NOOVF | `al=5: r=al+1` | `Long:6` |
| LNG_MUL_EXPR | `al=50000: r=al*50000` | `ERR6` |
| LNG_MOD_INTERMEDIATE | `al=2000000000: r=(al+al) Mod 7` | `ERR6` |
| BYTE_ADD_ASSIGN | `Dim ab As Byte: ab=200: ab=ab+100` | `ERR6` |
| BYTE_ADD_EXPR | `ab=200: r=ab+100` | `Integer:300` |
| VAR_INT_NOOVF | `Dim v: v=5: r=v+1` | `Integer:6` |
| VAR_INT_WIDEN | `v=32767: r=v+1` | `Long:32768` |
| VAR_LNG_WIDEN | `v=2000000000: r=v+v` | `Double:4000000000` |
| VAR_MUL_WIDEN | `v=50000: r=v*50000` | `Double:2500000000` |

## Derived rules (the spec target)

1. **Operation result type** = `numeric_join` of operand types, with `Byte` promoted to
   `Integer` (VBA has no `Byte` arithmetic). Integer literals are typed by value: `Integer`
   if within `-32768..32767`, else `Long`, etc.
2. **Fixed-type operands** (declared/literal `Integer`/`Long`/`Byte`): the operation is
   evaluated and **range-checked against the result type**; out of range → **run-time error 6
   "Overflow"**, raised *at the operation* (so intermediate overflow in a larger expression
   errors before later operators — see `LNG_MOD_INTERMEDIATE`). No silent widening.
3. **Variant operands**: the operation **widens** on overflow — `Integer`→`Long`→`Double`
   (`VAR_INT_WIDEN`, `VAR_LNG_WIDEN`, `VAR_MUL_WIDEN`). No error.
4. **Assignment to a declared fixed numeric target** narrows the value to the target type with
   a range check; out of range → **error 6** (`BYTE_ADD_ASSIGN`: `ab+100`→`Integer 300`, then
   `ab = …` narrows to `Byte` → error 6).

## Out of scope for bd-0d1y (noted)

- Full Variant numeric *subtype* fidelity for non-overflowing values: Excel preserves the
  smallest fitting subtype (`v=5: v+1` → `Integer:6`), whereas OxVba's Variant integers are
  `Long`-tagged, so `VAR_INT_NOOVF` would report `Long:6` (value matches; `TypeName` differs).
  This is numeric-subtype-tag fidelity, distinct from overflow semantics. Tracked separately.
- `Single`/`Double`/`Currency` overflow → error 6 (rarer; this bead targets `Integer`/`Long`/
  `Byte`/`LongLong` integer overflow, the `bd-0d1y` scope).

## Implementation (bd-0d1y)

Implemented as a `CoerceNumeric { slot, target }` bytecode opcode (an overflow *guard*: round
half-to-even → range-check → run-time **error 6** on out-of-range; in-range values pass
through unchanged). The compiler emits it via `insert_arithmetic_overflow_coercions`:

- around each fixed-integer arithmetic operation (`+`/`-`/`*`, unary `-`) whose result type is a
  fixed integer (so intermediate overflow errors at the operation, e.g. `(al+al) Mod 7`), and
- at each assignment into a declared fixed-integer target (so narrowing overflow like
  `ab = ab + 100` errors). Variant-typed operations are left to widen.

This matches every error and widen case in the table above. Reproduce the Excel ground truth
with `scripts/run-arith-overflow-oracle.ps1`; the OxVba side is regression-tested in
`crates/oxvba-vm/tests/vm_feature_coverage.rs` (`overflow_*`,
`fixed_integer_arithmetic_in_range_does_not_error`).

**Deferred (separate from this overflow gate):** in-range fixed-integer *results* keep their
current `Long`/`i32` carrier tag rather than being retagged to `Integer`/`Byte` — so
`VAR_INT_NOOVF`/`INT_NOOVF` report value-correct but tag as `Long` where Excel reports
`Integer`. Closing that subtype-tag gap would retag declared `Integer`/`Byte` slots (and require
regenerating the affected conformance goldens), and is tracked apart from `bd-0d1y`.
