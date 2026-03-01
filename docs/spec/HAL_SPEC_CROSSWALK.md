# HAL Spec Crosswalk (Foundation Anchors)

Status: `working-draft`  
Date: 2026-03-01

This map links HAL capabilities and implemented host-sensitive intrinsics to conformance candidate anchors extracted in:
- `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/conformance_items.jsonl`

## Capability Mapping

| HAL Capability | OxVba Surface | Foundation Anchor(s) | Notes |
|---|---|---|---|
| `UiInteraction` | `MsgBox`, `InputBox` | `CONF-discovered-ms-vbal-250520-f945507e-0337`, `...-0329` | Candidate extraction currently captures signature-level statements; detailed behavior remains for empirical refinement. |
| `EventPump` | `DoEvents` | `TBD-extraction` | Current pass does not provide a stable high-quality anchor row for `DoEvents` behavior; extraction improvement required. |
| `FileSystemIo` | `FreeFile`, file/seek/eof/lof family | `CONF-discovered-ms-vbal-250520-f945507e-0286` (`FreeFile`) | `EOF`/`LOF`/`Seek` behavior needs stronger extracted anchors in later spec pass. |
| `ProcessEnv` | `Shell`, `Dir`, `Environ` | `CONF-discovered-ms-vbal-250520-f945507e-0346`, `...-0282` | `Environ` signature/behavior mapping remains partial in current extraction. |
| `ComActivationDispatch` | `CreateObject`, `DispatchInvoke` | `CONF-discovered-ms-vbal-250520-f945507e-0325` + MS-OAUT family | Windows-only support declared by current HAL profile matrix. |
| `TimeLocale` | `Date`, `Time`, `Now`, locale-sensitive hooks | `CONF-discovered-ms-vbal-250520-f945507e-0252` (current placeholder anchor) | Needs tighter per-function anchor set in follow-up pass. |
| `DynamicLinking` | `Declare`/symbol loading pathways | `TBD-extraction` | Current extraction does not provide a robust claim set for HAL-level dynamic-link contracts yet. |
| `DiagnosticsTelemetry` | deterministic host diagnostics | OxVba internal contract | Internal capability for error/evidence surfaces; no external VBA spec clause. |

## Runtime Wiring Crosswalk

| Intrinsic | VM Instruction | Required Capability | Compile-time Gate Eligible |
|---|---|---|---|
| `Shell` | `Instruction::IntrinsicShellHost` | `ProcessEnv` | Yes |
| `Environ` | `Instruction::IntrinsicEnvironHost` | `ProcessEnv` | Yes |
| `Dir` | `Instruction::IntrinsicDirHost` | `ProcessEnv` | Yes |
| `CreateObject` | `Instruction::IntrinsicCreateObjectHost` | `ComActivationDispatch` | Yes |
| `DispatchInvoke` | `Instruction::IntrinsicDispatchInvokeHost` | `ComActivationDispatch` | Yes |

Code references:
- host preflight: `crates/oxvba-host/src/engine.rs` (`preflight_host_sensitive_support`)
- VM host call routing: `crates/oxvba-vm/src/interpreter.rs`

## Known Crosswalk Quality Issues

1. Several extracted anchors are still `candidate` quality and mostly signature-level.
2. High-semantic host behavior clauses are not yet fully represented in extracted conformance rows.
3. Cross-source mapping for COM behavior spans MS-VBAL + MS-OAUT; extraction run alignment still needs dedicated pass.
