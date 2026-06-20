# TestEventServer SAFEARRAY(VT_RECORD) vtable oracle

- Date: 2026-06-20
- Fixture: `tools/OxVba.TestEventServer`
- Type library: `tools/OxVba.TestEventServer/bin/Debug/net48/OxVba.TestEventServer.tlb`
- Purpose: deterministic side-effect-free external COM oracle for typed record SAFEARRAY vtable support.

## Commands

```text
./tools/OxVba.TestEventServer/register.ps1 -Configuration Debug -Scope CurrentUser
cargo test -p oxvba-com --lib registered_testeventserver_typed_record_safearray_uses_vtable_oracle -- --nocapture
cargo test -p oxvba-com --lib record_safearray -- --nocapture
```

## Observations

- `IOxVbaTestEventServer` now exposes typed record-array members:
  - `SumTypedRecordArray(TestRecord[] records) As Long`
  - `ReturnTypedRecordArray() As TestRecord[]`
  - `MutateTypedRecordArray(ByRef records As TestRecord[])`
- The exported typelib projects `SumTypedRecordArray` and `ReturnTypedRecordArray` as `SAFEARRAY(VT_RECORD)` with descriptor-backed `IRecordInfo` metadata.
- `registered_testeventserver_typed_record_safearray_uses_vtable_oracle` activates the registered COM server, recovers live member metadata from `IDispatch::GetTypeInfo`, builds a `ComMemberSpec`, and executes `SumTypedRecordArray` through the vtable path with an empty descriptor-backed record array.
- The focused record SAFEARRAY suite also proves descriptor-backed empty `SAFEARRAY(VT_RECORD)` lowering against the in-process vtable fixture.

## Result

The repo now has a deterministic external value oracle for descriptor-backed `SAFEARRAY(VT_RECORD)` vtable invocation. AcroBroker remains useful third-party descriptor evidence, but its side-effectful member is no longer the only record-array value-oracle path.
