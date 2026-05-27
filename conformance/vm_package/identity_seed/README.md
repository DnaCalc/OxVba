# VM Package Identity Seed Fixtures

These fixtures exercise the first executable-semantic-package identity evidence
surface. They are VM-runnable package fixtures, not JIT execution evidence.

Run with:

```powershell
cargo test -p oxvba-vm --test package_identity_fixtures -- --nocapture
```

The test output prints value snapshots plus package digest, bytecode digest,
slot counts, procedure identity fields, per-procedure slot descriptor digests,
slot descriptor tokens, signature descriptor digests for observed `CallProc`
targets, signature/call observation tokens, call-site descriptor digests, and
call-site descriptor observation tokens for each fixture. The array rows also
print array-shape descriptor digests and array-shape observation tokens. The
UDT rows also print UDT descriptor digests and UDT descriptor observation
tokens, plus selected lifecycle cleanup observation tokens for owning fields.
The object rows also print object descriptor digests and object descriptor
observation tokens. Carrier-layout and value-state rows are VM-visible through
package identity evidence and are covered by the VM package tests for primitive,
object, aggregate, Decimal96 Variant-subtype, Empty, Null, Error/CVErr,
Missing, Nothing, and vbNullString lanes.

The VM evidence also emits canonical descriptor identity rows sourced from the
compiler/package descriptor identity helpers, including bytecode, procedure,
signature, slot, call-site, array-shape, UDT, object, interop, lifecycle,
carrier-layout, and value-state families reached by current package evidence.
These rows are package identity evidence; they do not imply broader
descriptor-driven VM execution.

Bundle-backed VM package identity now also exposes project-context evidence
when an `OxBundle` v11 supplies it: project/module/reference/import facts,
module option and compile-context facts, source maps, native library facts,
host capability requirements, deterministic package diagnostics, and gap
classifications. These identity-seed fixtures remain focused on VM-runnable
package evidence; full import, native ABI, and host-policy behavior closure
belongs to later workset beads.

The VMR-02 rows cover primitive scalar, `String`/`BStr`, declared `Variant`,
and the current VM-runnable UDT field-alias shape. The UDT base slots now carry
`UdtFields` carrier hints, but execution still uses the existing flattened
field aliases.

The typed-carrier/value-state lane now records `OxBundle` v12
`CarrierLayoutDescriptor` and `ValueStateDescriptor` facts. Decimal remains a
`Decimal96` Variant subtype extension row rather than an ordinary declared slot
type. Propagation through operators, calls, properties, COM/native boundaries,
and error paths remains later work.

The VMR-03 row is VM-runnable evidence for current call lowering compared with
procedure signature metadata. It observes ByVal no-copyback, ByRef copyback,
Optional default materialization, ParamArray packing, property value ByVal
semantics, and function return-slot copyout. This is evidence over existing
bytecode lowering, not broad descriptor-driven call execution. Seed
`CallSiteDescriptor` and `ArgumentBindingDescriptor` rows now exist in package
metadata for top-level project calls. VMR-06 has one selected package-backed
call-entry behavior, but full expression/COM/native call-site coverage and
broader descriptor-driven VM behavior remain later work.
The call-site descriptor lane now records `OxBundle` v13 invocation syntax
(`Call` keyword, no-`Call`, expression-call, and synthetic property-assignment
forms), source argument evaluation order, and package-visible diagnostic policy
ownership for the current compiler-owned 448/449/450 invalid-call cases.

The VMR-04 row adds dedicated call-site descriptor evidence for ByRef variable
alias/writeback, ByRef expression temporary/no-writeback, a ByVal `Long` to
declared-`Double` call shape, explicit Optional default materialization,
Optional `Variant` missing-policy metadata, and empty/non-empty `ParamArray`
packs. It now records the first behavior-driving split: raw bytecode execution
preserves the old `VarType=2` entry observation for the declared-`Double`
callee, while package execution consumes the selected VMR-06 descriptors and
observes `VarType=5`. The remaining current VM limitation in this row is
Optional `Variant`: the descriptor says the missing policy is
`VariantMissingError448` while current VM lowering still materializes a default
local observed by the fixture as `VarType=2`. That gap is not treated as
VBA-compatible missing-argument behavior. The call-site evidence also records
the selected table row ids for the behavior-driving call-entry coercion:
`COERCE-CALL-BYVAL-DECLARED-TARGET`, `COERCE-LET-NUMERIC-WIDEN`, and runtime
helper `oxvba_runtime::coerce_to`.

`VMR04_BYREF_EXPRESSION_FORMS` narrows the ByRef expression evidence to
currently VM-runnable source forms: direct variable alias/writeback,
statement-level parenthesized force-ByVal, arithmetic expression temporary,
literal temporary, and function-result temporary. Property/default-member
result forms are not claimed by this seed because the current same-module
property call path does not compile as a callable procedure in this fixture
shape; that residual remains classified with the object/default-member call
descriptor work.

`VMR04_CALL_DIAGNOSTIC_DESCRIPTOR_BASELINE` is the positive VM-runnable
baseline for named argument mapping and the currently supported named fixed
argument plus positional `ParamArray` pack shape. `diagnostic_manifest.csv`
then records current compile-time diagnostics for missing required arguments,
wrong argument counts, unknown named arguments, duplicate mappings, positional
after named, and named `ParamArray` targets, with the intended 448/449/450
classification kept as evidence for later descriptor-driven call binding.
The positive descriptor evidence also emits diagnostic-policy tokens so later
VM-owned binding changes can prove when ownership intentionally moves away from
the current compiler diagnostics.

`VMR05_ARRAY_SHAPE_BOUNDS` is the first VM-runnable array shape descriptor
fixture. It records fixed/static and dynamic local array slots, compiler
generated fixed-array element slots, `Option Base 1` influence, explicit
`0 To 2` bounds, dynamic `ReDim 2 To 4` runtime SAFEARRAY bounds, and ByRef
copyback of the observed scalar results. The fixture now calls `LBound` and
`UBound` on fixed/static local arrays: raw bytecode execution still fails on
the unallocated base slot, while package execution resolves the rank-1
declared bounds through `ArrayShapeDescriptor`. Broader multi-rank,
bounds-error, lifecycle, and COM/native SAFEARRAY projection evidence remain
later work.

`VMR05_UDT_DESCRIPTOR_MEMBERS` is the first nominal UDT descriptor fixture. It
records descriptor ids, owning instances, field order, primitive field
carriers, nested UDT references, fixed-length string metadata, fixed array
field bounds, field-alias slots, fieldwise copy classification, and first
cleanup ownership flags. It intentionally uses the VM's current flattened field
alias syntax for fixed array fields; descriptor-backed UDT execution and
offset/layout consumption remain later VM work.

`VMR06-UDT-OWNING-FIELD-CLEANUP-001` adds the first lifecycle evidence lane for
owning UDT fields. `VMR02_UDT_FIELD_SLOTS` records variable `String` field
cleanup obligations for `Point.Caption`, while `VMR05_UDT_DESCRIPTOR_MEMBERS`
records fixed-length `String * 5` cleanup obligations for `Record.Name`. The
evidence is descriptor-backed and fixture-asserted, but it does not add an
explicit cleanup stack or change runtime carrier drop behavior.

`VMR05_OBJECT_DESCRIPTOR_IDENTITY` is the first object descriptor fixture. It
records a generic `Object` local with `Nothing` initial state and `ObjectRef`
carrier evidence without changing the value snapshot, which still observes the
current empty object slot state until object execution consumes descriptors.
The companion project-level test in `package_identity_fixtures.rs` exercises
the VM-capable route evidence for `Dim obj As New ThingImpl`, `Implements`
interface aliases, and imported COM `WithEvents` route metadata. Default
instances, `As New` slot activation policy, imported COM class/interface
descriptors, and descriptor-driven object/member execution remain later work.

The durable classification for those call-shape gaps lives in
`docs/spec/EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md` under
`VMR-04 Call Fixture Gap Classification`. Keep this fixture README descriptive;
do not use it as the owner for behavior-changing call-binding decisions.
