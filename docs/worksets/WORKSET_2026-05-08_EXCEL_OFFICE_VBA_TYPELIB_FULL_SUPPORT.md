# Workset: Excel / Office / VBA Typelib Full Support Audit

Date: 2026-05-08
Status: complete for the installed Office 16.0 / VBA 6.0 / VBIDE 5.3 audit slice

## Scope

Support the full installed Microsoft Excel, Microsoft Office, Visual Basic for Applications, and VBIDE typelibs as live COM typelibs loaded from registry/path metadata. This workset is evidence-backed by enumerating the actual installed typelibs rather than relying on fixture catalogs.

## Implemented support

- Added a Windows typelib shape audit harness:
  - `crates/oxvba-com/src/windows_typelib_loader.rs::audit_typelib_shapes`
  - `crates/oxvba-com/examples/typelib_audit.rs`
- Expanded typelib metadata lowering for Office/Excel/VBA shapes:
  - unsigned integer VARTYPEs (`VT_UI2`, `VT_UI4`, `VT_UINT`, `VT_UI8`)
  - signed byte (`VT_I1`)
  - narrow/wide string pointer VARTYPEs (`VT_LPSTR`, `VT_LPWSTR`)
  - safe/c arrays as Variant-compatible metadata (`VT_SAFEARRAY`, `VT_CARRAY`)
  - recursive `VT_PTR` handling
  - `VT_USERDEFINED` resolution through `ITypeInfo::GetRefTypeInfo`
  - enum user-defined types lowered as Long
  - alias user-defined types lowered through their alias target
  - record/interface/dispatch/coclass user-defined types lowered as Object
- Included module-scoped typelib functions in metadata enumeration, which is required for the VBA runtime typelib.
- Preserved the inherited IUnknown/IDispatch hidden-member filter for interface/dispatch types while not filtering module functions as inherited dispatch members.

## Evidence

Latest audit evidence:

- `docs/evidence/typelib_audit/office_excel_vba_20260508T202436/typelib_audit.csv`

The audit loaded and enumerated these installed typelibs:

| Library | Metadata members | Events | Type infos | Functions | Vars |
|---|---:|---:|---:|---:|---:|
| Excel 16.0 | 21328 | 166 | 1036 | 22492 | 2328 |
| Office 16.0 | 5279 | 42 | 521 | 5993 | 2855 |
| VBA 6.0 | 224 | 0 | 29 | 257 | 128 |
| VBIDE 5.3 | 396 | 5 | 64 | 480 | 50 |

No `unsupported_vt` rows remain in the final audit CSV.

## Boundaries

This completes metadata/audit support for the installed Excel/Office/VBA/VBIDE typelib feature shapes. It does not by itself claim arbitrary native vtable marshalling or complete semantic execution parity for every Office object member; dispatch execution remains governed by the existing COM invocation policy.
