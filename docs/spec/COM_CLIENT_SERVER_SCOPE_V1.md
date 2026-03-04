# COM Client/Server Scope V1

Status: `design-draft`  
Date: 2026-03-04  
Primary scope: Windows (`HalProfileId::Windows`)  
Related ladder: `docs/worksets/PROFILE_LADDER_2026-03-04_MACH1000_V287_V306_COM_FORMAL_SCAFFOLD.md`

## 1. Objective

Define a formal, implementation-ready scope for OxVba COM support in two roles:

1. COM client: OxVba code calls external COM automation servers.
2. COM server: OxVba runtime exposes automation-visible objects to external COM hosts.

This scope follows `CHARTER.md` value ordering:

1. robustness
2. compatibility
3. performance

## 2. Normative Source Set

Canonical roots:

- `docs/FOUNDATION_SPEC_REFERENCE.md`
- `../Foundation/reference/runs/20260301-ms-vbal-pass07/outputs/`
- `../Foundation/reference/runs/20260301-ms-oaut-pass02/outputs/`
- `../Foundation/reference/runs/20260301-ms-dtyp-pass02/outputs/`

Primary anchor families already used in OxVba docs:

- MS-VBAL:
  - `CONF-discovered-ms-vbal-250520-f945507e-0325` (`CreateObject` signature family)
  - `CONF-discovered-ms-vbal-250520-f945507e-0091`, `...-0092`, `...-0093` (implementation-defined external declaration/selection controls)
  - `CONF-discovered-ms-vbal-250520-f945507e-0056`, `...-0097`, `...-0140`, `...-0143` (`WithEvents`/`Implements` class constraints)
- MS-OAUT:
  - `CONF-discovered-ms-oaut-240423-b76f9b41-0010`, `...-0011` (`VT_BYREF` legality)
  - `...-0023..0029` (`BSTR`, `IDispatch*`, `IUnknown*`, `VARIANT` compatibility)
  - `...-0042`, `...-0050..0052` (`SAFEARRAY` constraints)
  - `CONF-discovered-ms-oaut-210625-4fcc3347-0080..0084` (`IDispatch::Invoke` output obligations)
- MS-DTYP:
  - `CONF-discovered-ms-dtyp-241119-518a70cb-0002..0005`, `...-0007..0009` (pointer/string ABI requirements)

Source-quality note:

- Some extracted items remain `candidate` quality; parity claims must keep explicit deferred-oracle links until higher-confidence extraction and empirical foldback complete.

## 3. Design Decisions (V1)

### D1. Platform Support

- Windows: COM client+server are in scope.
- Linux/macOS/WASM/null: remain deterministic unsupported for COM operations in this series.

### D2. Boundary Ownership

- COM client behavior remains HAL-governed (`ComHal`, plus dynamic-link boundaries where required).
- COM server behavior is runtime/host-facing and is not modeled as a HAL capability for non-Windows profiles.
- Result: avoid forcing a cross-platform abstraction for a Windows-only ABI surface.

### D3. Apartment Model Policy

- Default policy for COM-enabled engine instances is STA-oriented execution.
- Initial implementation strategy: one dedicated COM thread per engine/runtime host process (or host-injected STA thread), explicit apartment initialization lifecycle, deterministic rejection for unsupported apartment policy modes.
- This is an OxVba implementation decision for compatibility/robustness; it is tracked as implementation-defined until empirical parity evidence closes.

### D4. Registration Strategy

- Two explicit lanes are required:
  - registration-free test lane (deterministic, CI-suitable),
  - registered ProgID/CLSID lane (host-realistic).
- Both lanes must share the same contract/error mapping semantics.

### D5. Error Surface

- Every COM boundary failure must map to deterministic OxVba diagnostics (`HalError` and VM/host error routes).
- No silent fallback from COM to projection mode for COM-enabled profiles once a call is routed to native COM lanes.

## 4. Capability and Maturity Tiers

### Client tiers

- `C0` (existing): deterministic token projection only.
- `C1`: native activation + scalar invoke (`CreateObject`, `GetIDsOfNames`, `Invoke` with scalar arguments).
- `C2`: byref/optional/named argument invoke parity subset (`DISPPARAMS`, `ArgErr`, `ExcepInfo`, `VarResult` handling).
- `C3`: array/object boundary subset (`SAFEARRAY`, interface pointers, richer variant coercion lanes).

### Server tiers

- `S0` (current): no native COM server behavior.
- `S1`: minimal Rust automation server scaffold (`IUnknown` + `IDispatch`) with deterministic echo/math methods.
- `S2`: OxVba host/server bridge exposing selected runtime entrypoints via COM dispatch.
- `S3`: class-module-aligned exposure model and host policy controls for surface publication.

## 5. Formal Contract Shape (Pre/Post Conditions)

### 5.1 Activation (`CreateObject`)

Preconditions:

- Windows profile active.
- COM activation policy enabled.
- Apartment initialized for COM lane.

Postconditions:

- success: stable object handle token bound to valid COM identity.
- failure: deterministic error family with stable code, operation, and source metadata.

### 5.2 Dispatch invoke

Preconditions:

- object token resolves to a live COM object.
- member token/name resolves deterministically under case policy.
- argument pack contract satisfied.

Postconditions:

- success: return token value with explicit mapping contract.
- failure: deterministic translation of HRESULT/EXCEPINFO/ArgErr to OxVba diagnostics and `Err` model.

### 5.3 Lifetime invariants

- object handles must preserve reference-lifetime safety:
  - no use-after-release,
  - no double-release,
  - deterministic cleanup at engine shutdown.

### 5.4 Server registration/exposure

Preconditions:

- host policy enables COM server publication.
- class/object metadata passes exposure checks.

Postconditions:

- COM clients can obtain and invoke exposed object contract for in-scope members.
- unsupported shape or policy denial fails deterministically before exposure.

## 6. Test Scaffolding Targets (Rust-first)

Required scaffolds in this series:

1. Small COM automation test servers in Rust (expandable method sets).
2. Small COM host/client harnesses in Rust to drive:
   - direct client calls,
   - server exposure calls,
   - roundtrip call paths through OxVba runtime.
3. Deterministic fixture corpus under `conformance/` for COM client/server lanes.

## 7. Out of Scope (This Series)

- DCOM/remoting semantics.
- Full type-library import/export parity.
- COM events/connection points full parity.
- Non-Windows COM emulation.

## 8. Deferred/Uncertain Topics

Track as implementation-defined or deferred-oracle topics:

1. Apartment/subthread interactions when host already owns COM initialization.
2. Exact named/optional argument packing parity for broad `Invoke` shapes.
3. Class-module exposure policy vs host-injected project/module metadata evolution.
4. Registration-free server loading constraints under varied CI environments.

Tracking files:

- `docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`
- `docs/evidence/hal/HAL_UNCERTAINTY_REGISTER.md`
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`

