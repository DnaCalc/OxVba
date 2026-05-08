# Workset — Typelib To Internal Interface Binding

Date: 2026-05-08
Status: in-progress; strict Access/JET natural early-bound slice is green, broader descriptor/ABI completion remains

## Objective

Bind imported COM type libraries into the new COM-shaped internal OxVba object/interface descriptor model so true natural early-bound VBA COM calls compile and execute through the same call representation used by pure OxVba objects.

This is Workset 2 in the Access/JET strict early-binding recovery sequence.

## Dependency

This workset must not be treated as complete until Workset 1 provides:

- descriptor-backed internal object/interface model;
- typed known-interface member call representation;
- optimized cached dynamic dispatch representation;
- default/indexed property semantics;
- property get/let/set call-kind preservation;
- object result assignment-intent semantics.

## Rationale

The current imported COM lane is mixed:

- `.basproj` can carry COM references;
- real typelibs can be loaded;
- imported declarations such as `Dim cn As New ADODB.Connection` can be recognized;
- some calls such as `cn.Open(...)` and `cn.Execute(...)` can use metadata-backed member resolution;
- unsupported shapes still fall back to explicit `DispatchInvoke(...)` or fail.

That is not true end-to-end early-bound VBA COM. The strict Access/JET test currently fails on natural syntax such as:

```vb
Set fieldName = rs.Fields("Name")
nameValue = fieldName.Value
```

with a typelib arity/member-shape diagnostic.

The same workset must also support the common DAO/ADO recordset bang-member idiom:

```vb
MyValue = MyRecordset!ColName
```

For this expression, `!ColName` is not a field access on an OxVba UDT. It is Access/VBA recordset shorthand: use the recordset's default collection/member path to look up item `"ColName"`, then because the assignment lacks `Set`, dereference the returned object's default value property and assign that scalar value.

## Design Direction

Imported COM typelibs should become another producer of the same internal descriptors used by OxVba-native classes and interfaces.

For each imported typelib:

- project reference resolver loads and normalizes library/type/interface descriptors;
- coclasses map to activation identities and default/source interfaces;
- interfaces map to internal interface descriptors;
- members map to internal member descriptors with DISPIDs, vtable slots where available, invoke kind, property flags, default/indexed metadata, optional/named argument metadata, return type, and parameter descriptors;
- recordset/bang-member syntax maps to the same descriptor machinery as an indexed/default collection lookup plus value-context default-member dereference;
- imported object values carry native COM receiver state plus projected internal interface identity;
- known imported receiver calls lower to typed internal interface/member calls;
- runtime dispatch adapter chooses native COM vtable or native COM `IDispatch`/`IDispatchEx` strategy according to available metadata and policy.

## Dispatch Strategy

OxVba should support both:

1. **Imported early-bound known-interface call**
   - Lowered from typed source, e.g. `Dim cn As ADODB.Connection` and `cn.Open(...)`.
   - Uses the same call representation as pure OxVba known-interface calls.
   - Runtime adapter may invoke native COM vtable where safe or descriptor-backed `IDispatch` where vtable ABI support is not yet complete.

2. **Imported late-bound/dynamic call**
   - Lowered when receiver static type is `Variant`, `Object`, or otherwise unknown.
   - Uses OxVba dynamic dispatch plan cache.
   - If receiver is native COM, dispatch plan caches name-to-DISPID and argument-shape binding before calling native `IDispatch`.
   - If receiver is OxVba-native, dispatch plan uses internal descriptor lookup/cache without native COM overhead.

## Primary Red Test

The strict Access/JET test in `crates/oxvba-host/tests/com_early_project_end_to_end.rs` is the primary red-to-green target for the first Access/JET slice:

```rust
strict_early_bound_project_executes_registered_access_jet_ado_database_subset
```

It intentionally uses natural VBA COM syntax:

```vb
Dim catalog As New ADOX.Catalog
Dim cn As New ADODB.Connection
Dim rs As ADODB.Recordset
Dim fieldName As ADODB.Field
Dim fieldScore As ADODB.Field

Call catalog.Create(connection)
Call cn.Open(connection, "", "", 0)
Call cn.Execute("CREATE TABLE ...", 0, 0)
Set rs = cn.Execute("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 0, 0)
Set fieldName = rs.Fields("Name")
Set fieldScore = rs.Fields("Score")
nameValue = fieldName.Value
scoreValue = fieldScore.Value

' Required shorthand equivalent for value context:
nameValue = rs!Name
scoreValue = rs!Score
```

Current status:

```text
cargo test -p oxvba-host --test com_early_project_end_to_end strict_early_bound_project_executes_registered_access_jet_ado_database_subset -- --nocapture
```

passes on the Windows machine used for this run, including `rs.Fields("Name").Value`, `Set bangFieldName = rs!Name` object-context shorthand, and `rs!Name` / `rs!Score` value-context shorthand. The broader early-bound host binary also passed:

```powershell
cargo test -p oxvba-host --test com_early_project_end_to_end -- --test-threads=1
```

Result: 121 passed, 0 failed.

The implementation currently lowers natural source syntax into the existing COM dispatch bridge internally while preserving strict source shape (no `DispatchInvoke` in the test source). Broader descriptor-backed internal ABI unification and generalized typelib binding remain in progress.

Additional descriptor projection progress:

- `oxvba-com::runtime_class_descriptor_from_typelib_metadata(...)` maps imported `TypeLibMetadataBlob` member metadata into the shared runtime descriptor model (`RuntimeClassDescriptor` / `RuntimeInterfaceDescriptor` / `RuntimeMemberDescriptor`).
- The projection preserves dispatch ids, invoke kind, default-member flag, arity, and optional vtable slot metadata. `dual_dispatch` is only asserted when the metadata carries an explicit vtable slot.
- Live Windows typelib loading now captures `FUNCDESC::oVft` into `TypeLibMemberMetadata::vtable_slot`; fixture/catalog metadata remains test-scoped behind `cfg(test)` / `fixture-typelibs`, with no hardcoded ADODB/ADOX/Scripting/Excel member catalog in production paths.
- `typelib_metadata_projects_to_runtime_dispatch_descriptor` covers the projection.
- Validation: `cargo test -p oxvba-com --quiet` -> 93 passed.
- Additional Access/JET ambiguity validation: `cargo test -p oxvba-host --test com_early_project_end_to_end strict_early_bound_project_executes_registered_access_jet_ado_database_subset -- --nocapture` -> 1 passed, with strict source covering both `Set bangFieldName = rs!Name` and `bangNameValue = rs!Name`.

## Scope

### In scope

1. Convert loaded COM typelib type/member metadata into internal interface descriptors.
2. Preserve typelib DISPIDs and vtable slot metadata in descriptor records.
3. Represent imported default and indexed properties correctly.
4. Support returned imported object typing across assignments and chained calls.
5. Support optional/named/default arguments sufficiently for ADO/ADOX Access/JET flow.
6. Support recordset bang-member syntax (`MyRecordset!ColName`) as default collection lookup by field name followed by value-context default-property dereference when the expression is used without `Set`.
7. Cache native COM name lookup and DISPID binding for dynamic imported calls.
8. Preserve assignment intent for object-returning imported members (`Set` vs value assignment), including bang-member behavior where `Set field = rs!Name` preserves the field object but `value = rs!Name` reads the field's default value.
9. Make the strict Access/JET natural early-bound test pass without `DispatchInvoke` in source.

### Out of scope

- General parity for every COM library/member shape.
- COM server packaging/export of OxVba classes. That is enabled by Workset 1 descriptors but should be tracked separately if packaging/registration is in scope.
- Cross-platform COM runtime support.

## Completion Criteria

This workset is complete only when:

- imported COM library descriptors flow through the same internal interface model as pure OxVba objects;
- the strict Access/JET early-bound test passes without `DispatchInvoke` in source;
- the strict Access/JET test includes and passes `MyRecordset!ColName` value-context shorthand semantics;
- mixed imported-COM showcase language is either removed or clearly separated from true early-bound evidence;
- dynamic imported COM calls cache name/DISPID lookup;
- docs explain vtable-vs-dispatch policy for imported native COM calls;
- no previous COM conformance lanes regress.
