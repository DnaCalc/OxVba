# Native-Ready Value Substrate v1

Status: `recovered-baseline` (phase-3 executable proof restored)
Date: 2026-04-30; recovery update 2026-05-02
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

`RuntimeValue` is not a future-facing semantic substrate. It has been removed
from active Rust source and must not be reintroduced into execution, snapshot,
or presentation APIs.

## Phase-2 Baseline

The 2026-05-02 recovery gate establishes this baseline:

- active fake HIR/MIR/CFG crate APIs are gone;
- active Rust source has zero `RuntimeValue|runtime_value` matches;
- normal VM, JIT, and host snapshot/invoke/debug/immediate/embedded surfaces use
  retained `Variant` or named DTOs;
- historical RuntimeValue evidence remains in docs only and is not an approved
  active API residual.

Evidence:
[`../evidence/native_ready/RUNTIMEVALUE_ACTIVE_RUST_SOURCE_REMOVAL_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_ACTIVE_RUST_SOURCE_REMOVAL_2026-05-01.md)
and
[`../evidence/native_ready/NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md`](../evidence/native_ready/NATIVE_READY_RECOVERY_AUDIT_2026-05-02.md).

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
[`../evidence/native_ready/EXACT_CARRIER_EXPECTATIONS_2026-05-01.md`](../evidence/native_ready/EXACT_CARRIER_EXPECTATIONS_2026-05-01.md)
and the recovery proof
[`../evidence/native_ready/VALUE_NUMERIC_UDT_RECOVERY_EXECUTABLE_TESTS_2026-05-02.md`](../evidence/native_ready/VALUE_NUMERIC_UDT_RECOVERY_EXECUTABLE_TESTS_2026-05-02.md).

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

V1 storage is descriptor-backed logical aggregation over retained `Variant`
field slots. A UDT variable is not a native struct overlay, and the current
implementation path may preserve flattened field slots while adding descriptor
identity, field descriptors, and descriptor-driven copy/init rules.

Internal UDT semantics are separate from native ABI layout. A future ABI layer
may materialize a UDT as a platform struct for `Declare` or native exports, but
that materialization is not the canonical internal value model.

Detailed model path:
[`../evidence/native_ready/UDT_DESCRIPTOR_MODEL_PATH_2026-05-01.md`](../evidence/native_ready/UDT_DESCRIPTOR_MODEL_PATH_2026-05-01.md).
Executable recovery coverage for nested field access, same-type whole copy, and
same-shape cross-type rejection is recorded in
[`../evidence/native_ready/VALUE_NUMERIC_UDT_RECOVERY_EXECUTABLE_TESTS_2026-05-02.md`](../evidence/native_ready/VALUE_NUMERIC_UDT_RECOVERY_EXECUTABLE_TESTS_2026-05-02.md).

## Residual Boundaries

### Historical RuntimeValue residuals

The old RuntimeValue compatibility residual register is historical after
`bd-0w46`; no active Rust RuntimeValue API residual is approved for future
native-facing planning. If a future compatibility surface is intentionally
reintroduced, it must be created as a new explicit blocker with semver and
native-readiness impact.

Historical register:
[`../evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md`](../evidence/native_ready/RUNTIMEVALUE_BRIDGE_PUBLIC_API_BLOCKERS_2026-05-01.md).

### Native/UDT residuals

The following remain explicitly outside this v1 substrate until a later workset
expands scope:

- arbitrary native UDT-byref ABI parity,
- native struct-overlay parity,
- platform packing/alignment equivalence for all UDT shapes,
- COM record/UDT transport parity,
- direct native PE/ELF code generation.

Classification evidence:
[`../evidence/native_ready/UDT_NATIVE_ABI_RESIDUAL_CLASSIFICATION_2026-05-01.md`](../evidence/native_ready/UDT_NATIVE_ABI_RESIDUAL_CLASSIFICATION_2026-05-01.md).

## Acceptance Gates

- Active runtime APIs use `Variant` or explicit DTOs, with zero active Rust
  `RuntimeValue|runtime_value` matches.
- Numeric helper families are `Variant`-native and backed by executing tests
  (`bd-9xmu.3.4` recovery).
- Mixed numeric result behavior is matrixed and backed by executing tests
  (`bd-9xmu.3.5` recovery).
- Exact `Currency`, `Decimal`, `Date`, and Boolean carrier behavior is pinned
  (`bd-9xmu.3.6`).
- UDT semantics are descriptor-backed or explicitly blocked (`bd-9xmu.3.7`).
- Native ABI materialization is documented separately from internal UDT storage
  (`bd-9xmu.3.8`).

