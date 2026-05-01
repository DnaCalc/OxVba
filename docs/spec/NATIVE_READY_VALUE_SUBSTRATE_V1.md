# Native-Ready Value Substrate v1

Status: `working-draft`
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

`RuntimeValue` is not a future-facing semantic substrate. It must be removed
from active APIs or isolated as an explicitly temporary compatibility blocker.

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

The following remain explicitly outside this v1 substrate until a later workset
expands scope:

- arbitrary native UDT-byref ABI parity,
- native struct-overlay parity,
- platform packing/alignment equivalence for all UDT shapes,
- direct native PE/ELF code generation.

## Acceptance Gates

- Active runtime APIs use `Variant` or explicit DTOs, not `RuntimeValue`.
- Numeric helper families are `Variant`-native.
- UDT semantics are either descriptor-backed or explicitly blocked.
- Native ABI materialization is documented separately from internal UDT storage.

