# HAL Spec Crosswalk (Foundation Anchors)

Status: `working-draft`  
Date: 2026-03-02

This map links HAL capabilities and implemented host-sensitive intrinsics to conformance candidate anchors extracted in:
- `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/conformance_items.jsonl`
- `../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/conformance_items.jsonl`
- `../Foundation/reference/runs/20260301-ms-dtyp-pass02/outputs/conformance_items.jsonl`

## Capability Mapping

| HAL Capability | OxVba Surface | Foundation Anchor(s) | Notes |
|---|---|---|---|
| `UiInteraction` | `MsgBox`, `InputBox` | `CONF-discovered-ms-vbal-250520-f945507e-0337`, `...-0329` | Candidate extraction currently captures signature-level statements; detailed behavior remains for empirical refinement. |
| `EventPump` | `DoEvents` | `TBD-extraction` | Current pass does not provide a stable high-quality anchor row for `DoEvents` behavior; extraction improvement required. |
| `FileSystemIo` | `FreeFile`, file/seek/eof/lof family | `CONF-discovered-ms-vbal-250520-f945507e-0286` (`FreeFile`) | `EOF`/`LOF`/`Seek` behavior needs stronger extracted anchors in later spec pass. |
| `ProcessEnv` | `Shell`, `Dir`, `Environ` | `CONF-discovered-ms-vbal-250520-f945507e-0346`, `...-0282` | `Environ` signature/behavior mapping remains partial in current extraction. |
| `ComActivationDispatch` | `CreateObject`, `DispatchInvoke` | `CONF-discovered-ms-vbal-250520-f945507e-0325` + MS-OAUT family | Windows-only support declared by current HAL profile matrix. |
| `TimeLocale` | `Date`, `Time`, `Now`, locale-sensitive hooks | `CONF-discovered-ms-vbal-250520-f945507e-0252` (current placeholder anchor) | Needs tighter per-function anchor set in follow-up pass. |
| `DynamicLinking` | `Declare`/symbol loading pathways + marshaling boundary | MS-VBAL `...-0088`, `...-0090`, `...-0091`, `...-0092`, `...-0093`, `SPEC-...-01617`, `...-01624..01627`; MS-OAUT `...-0010`, `...-0011`, `...-0015`, `...-0016`, `...-0023..0029`, `...-0042`, `...-0050..0060`, `...-0080..0084`; MS-DTYP `...-0002..0005`, `...-0007..0009` | Cross-source cluster: declaration surface is implementation-defined in parts (VBAL), marshaling legality is strongly normative in OAUT/DTYP. Extraction quality is mixed (`candidate` rows, OCR artifacts). |
| `DiagnosticsTelemetry` | deterministic host diagnostics | OxVba internal contract | Internal capability for error/evidence surfaces; no external VBA spec clause. |

## DynamicLinking Deep Links (Declare + Marshaling)

### MS-VBAL declaration/selection anchors

| Anchor ID | Summary |
|---|---|
| `CONF-discovered-ms-vbal-250520-f945507e-0088` | `#ordinal` alias must match integer-literal grammar. |
| `CONF-discovered-ms-vbal-250520-f945507e-0090` | non-ordinal alias syntax is implementation-defined. |
| `CONF-discovered-ms-vbal-250520-f945507e-0091` | implementation may add external-procedure declaration restrictions. |
| `CONF-discovered-ms-vbal-250520-f945507e-0092` | implementation may add restrictions when `PtrSafe` is absent. |
| `CONF-discovered-ms-vbal-250520-f945507e-0093` | alias-based procedure selection is implementation-defined. |
| `SPEC-discovered-ms-vbal-250520-f945507e-01617` | case-sensitivity of external procedure names is implementation-defined. |
| `SPEC-discovered-ms-vbal-250520-f945507e-01624..01627` | library/alias/proc-name selection mechanism is implementation-defined. |

### MS-OAUT marshaling anchors

| Anchor ID | Summary |
|---|---|
| `CONF-discovered-ms-oaut-240423-b76f9b41-0010`, `...-0011` | `VT_BYREF` legality and forbidden combinations with `VT_EMPTY`/`VT_NULL`. |
| `...-0015`, `...-0016` | variant flags that require missing data payload. |
| `...-0023..0029` | `BSTR`, `IDispatch*`, `HRESULT`, `VARIANT_BOOL`, `VARIANT`, `IUnknown*` compatibility constraints. |
| `...-0042`, `...-0050..0052` | `SAFEARRAY` typing and byref rules. |
| `...-0058..0060` | SAFEARRAY element typing for BSTR/IUnknown/IDispatch. |
| `CONF-discovered-ms-oaut-210625-4fcc3347-0080..0084` | `IDispatch::Invoke` output obligations (`VarResult`, `ExcepInfo`, `ArgErr`). |

### MS-DTYP support anchors

| Anchor ID | Summary |
|---|---|
| `CONF-discovered-ms-dtyp-241119-518a70cb-0002`, `...-0007`, `...-0008` | UTF-16 pointer string definitions (`LPCWSTR`/`LPWSTR`). |
| `CONF-discovered-ms-dtyp-241119-518a70cb-0003` | 8-bit pointer string (`LPSTR`) definition. |
| `...-0004`, `...-0009` | pointer strings require explicit string/length semantics. |
| `...-0005` | character format must be protocol-specified. |

## Runtime Wiring Crosswalk

| Intrinsic | VM Instruction | Required Capability | Compile-time Gate Eligible |
|---|---|---|---|
| `Shell` | `Instruction::IntrinsicShellHost` | `ProcessEnv` | Yes |
| `Environ` | `Instruction::IntrinsicEnvironHost` | `ProcessEnv` | Yes |
| `Dir` | `Instruction::IntrinsicDirHost` | `ProcessEnv` | Yes |
| `CreateObject` | `Instruction::IntrinsicCreateObjectHost` | `ComActivationDispatch` | Yes |
| `DispatchInvoke` | `Instruction::IntrinsicDispatchInvokeHost` | `ComActivationDispatch` | Yes |
| `Declare` call | `Instruction::IntrinsicInvokeSymbolHost` | `DynamicLinking` | Yes |

Code references:
- host preflight: `crates/oxvba-host/src/engine.rs` (`preflight_host_sensitive_support`)
- VM host call routing: `crates/oxvba-vm/src/interpreter.rs`

## Known Crosswalk Quality Issues

1. Several extracted anchors are still `candidate` quality and include OCR noise.
2. High-semantic host behavior is often implementation-defined in MS-VBAL; explicit OxVba policy/descriptor documentation remains required.
3. Dynamic-link marshaling obligations span MS-VBAL + MS-OAUT + MS-DTYP and require multi-source clause mapping.
