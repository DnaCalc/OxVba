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
call-site descriptor observation tokens for each fixture.

The VMR-02 rows cover primitive scalar, `String`/`BStr`, declared `Variant`,
and the current VM-runnable UDT field-alias shape. They do not claim nominal
UDT aggregate descriptors; that remains owned by the later UDT descriptor
evidence work.

The VMR-03 row is VM-runnable evidence for current call lowering compared with
procedure signature metadata. It observes ByVal no-copyback, ByRef copyback,
Optional default materialization, ParamArray packing, property value ByVal
semantics, and function return-slot copyout. This is evidence over existing
bytecode lowering, not descriptor-driven call execution. Seed
`CallSiteDescriptor` and `ArgumentBindingDescriptor` rows now exist in package
metadata for top-level project calls; full expression/COM/native call-site
coverage and descriptor-driven VM behavior remain later work.

The VMR-04 row adds dedicated call-site descriptor evidence for ByRef variable
alias/writeback, ByRef expression temporary/no-writeback, a ByVal `Long` to
declared-`Double` call shape, explicit Optional default materialization,
Optional `Variant` missing-policy metadata, and empty/non-empty `ParamArray`
packs. It intentionally records two current VM limitations: the ByVal
declared-`Double` callee observes `VarType=2` at entry instead of a coerced
Double value, and the Optional `Variant` descriptor says the missing policy is
`VariantMissingError448` while current VM lowering still materializes a default
local observed by the fixture as `VarType=2`. Those gaps are not treated as
VBA-compatible call-coercion or missing-argument behavior.

`VMR04_BYREF_EXPRESSION_FORMS` narrows the ByRef expression evidence to
currently VM-runnable source forms: direct variable alias/writeback,
statement-level parenthesized force-ByVal, arithmetic expression temporary,
literal temporary, and function-result temporary. Property/default-member
result forms are not claimed by this seed because the current same-module
property call path does not compile as a callable procedure in this fixture
shape; that residual remains classified with the object/default-member call
descriptor work.

The durable classification for those call-shape gaps lives in
`docs/spec/EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md` under
`VMR-04 Call Fixture Gap Classification`. Keep this fixture README descriptive;
do not use it as the owner for behavior-changing call-binding decisions.
