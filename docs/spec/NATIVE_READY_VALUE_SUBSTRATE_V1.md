# Native-Ready Value Substrate v1

Status: `locked-baseline`
Date: 2026-04-30
Scope owner: OxVBA runtime/compiler/native-readiness

## Purpose

Define the value substrate that native-facing OxVBA work is allowed to depend
on after the cleanup rebase.

## Canonical Carrier

The canonical execution and snapshot carrier is retained `Variant`.

Required retained carriers:

- `Empty`
- `Null`
- `Error`
- `Boolean`
- `Byte`
- `Integer`
- `Long`
- `LongLong`
- `LongPtr` where represented as pointer-sized integer semantics
- `Single`
- `Double`
- `Currency`
- `Decimal`
- `Date`
- `String` / `BStr`
- `ObjectRef`
- `SafeArray`

`RuntimeValue` is not a future-facing semantic substrate. It is not re-exported
from the runtime root and must not be introduced into new execution, snapshot,
or presentation APIs. Existing uses are only allowed in the residual boundaries
listed below and must be retired or explicitly public-API-blocked by the phase-3
bridge-retirement bead.

## Phase-2 Baseline

The 2026-05-01 phase-2 search gate established this baseline:

- active fake HIR/MIR/CFG crate APIs are gone;
- launcher/web/language-service presentation crates have no `RuntimeValue`
  matches;
- normal VM, JIT, and host snapshot/invoke/debug/immediate/embedded surfaces use
  retained `Variant` or named DTOs;
- remaining `RuntimeValue` occurrences are compatibility, tests, evidence, or
  bridge residuals tracked by
  [`../evidence/native_ready/RUNTIMEVALUE_IR_SEARCH_GATE_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_IR_SEARCH_GATE_2026-05-01.md).

## Numeric Semantics Direction

Numeric behavior must be specified by tag-aware operation matrices. Each matrix
row must record:

- left tag,
- right tag where applicable,
- operation,
- result tag,
- overflow behavior,
- rounding/truncation behavior,
- `Null` behavior,
- `Error`/`CVErr` behavior,
- string-coercion behavior,
- oracle/spec/implementation-defined source.

Native code may inline numeric operations only when the operation matrix proves
the specialization is equivalent to the retained-`Variant` helper path.

## Exact Scalar Carrier Rules

- `Currency` is exact scaled `i64` with scale 10,000.
- `Decimal` is exact `Decimal96` parts plus scale/sign.
- `Date` is a date-tagged `f64`; generic arithmetic currently returns `Double`
  unless a date-specific helper re-tags the result.
- `Boolean` preserves a Boolean carrier; numeric coercion uses VBA truth values
  (`True = -1`, `False = 0`).

Detailed regression evidence is in
[`../evidence/native_ready/EXACT_CARRIER_EXPECTATIONS_2026-05-01.md`](../evidence/native_ready/EXACT_CARRIER_EXPECTATIONS_2026-05-01.md).

## UDT Direction

UDTs need descriptor-backed semantic representation before native-facing code
may treat them as structured values.

Required model concepts:

- `UdtTypeId`
- `UdtFieldId`
- declared field name
- field declared type
- field order
- optional array bounds
- nested UDT reference
- semantic copy/init rules

Internal UDT semantics are separate from native ABI layout. A future ABI layer
may materialize a UDT as a platform struct for `Declare` or native exports, but
that materialization is not the canonical internal value model.

## Residual Boundaries

### RuntimeValue compatibility residuals

The following `RuntimeValue` residuals are approved only as compatibility or
public-API-blocker candidates:

- `oxvba_runtime::compat` and subordinate compatibility helper modules such as
  `coerce::compat` and `pointer_helpers::compat`;
- legacy VM/JIT/host extension traits and DTOs under explicit `compat` modules;
- HAL legacy trait methods that still pair with retained `_variant` companions;
- COM projection helpers under `oxvba_com::compat` plus current COM
  model/dynamic-object bridge methods until bridge retirement resolves them;
- tests and evidence documents that assert or classify legacy projection
  behavior.

No native-facing planning may depend on these residuals as normal value
semantics. Bead `bd-9xmu.3.2` recorded the public-API blocker register in
[`../evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md);
those blockers must be retired or carried into `CURRENT_BLOCKERS.md` before the
umbrella terminal gate can close.

### Native/UDT residuals

The following remain explicitly outside this v1 substrate until a later workset
expands scope:

- arbitrary native UDT-byref ABI parity,
- native struct-overlay parity,
- platform packing/alignment equivalence for all UDT shapes,
- direct native PE/ELF code generation.

## Acceptance Gates

- Active runtime APIs use `Variant` or explicit DTOs; any `RuntimeValue`
  residual is explicit compatibility/public-API-blocker work.
- Numeric helper families are `Variant`-native (`bd-9xmu.3.4`).
- Mixed numeric result behavior is matrixed and tested (`bd-9xmu.3.5`).
- Exact `Currency`, `Decimal`, `Date`, and Boolean carrier behavior is pinned
  (`bd-9xmu.3.6`).
- UDT semantics are descriptor-backed or explicitly blocked (`bd-9xmu.3.7`).
- Native ABI materialization is documented separately from internal UDT storage
  (`bd-9xmu.3.8`).

