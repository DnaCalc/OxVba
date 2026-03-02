# HAL Declare ABI + Marshaling Specification V1

Status: `working-draft`  
Step lineage: `v191`, `v212..v218`, post-`v226` formalization pass  
Date: 2026-03-02

## 1. Charter Alignment

This specification follows `CHARTER.md` priority order:
1. robustness,
2. compatibility,
3. performance.

For `Declare` and marshaling, this means:
- deterministic failure and diagnostics are mandatory,
- compatibility claims are layered by profile/runtime-class and evidence level,
- native ABI performance work is permitted only inside explicit contract boundaries.

## 2. Objective

Define a formal HAL contract for external procedure declarations and argument/result marshaling that is:
- cross-referenced to canonical source families,
- explicit about implementation-defined regions,
- enforceable through clause IDs and conformance plans.

## 3. Normative Source Anchors (Current Working Set)

Primary source root:
- `../Foundation/reference`

### 3.1 MS-VBAL anchors (`Declare` language surface)

| Anchor ID | Summary | Role in OxVba HAL contract |
|---|---|---|
| `CONF-discovered-ms-vbal-250520-f945507e-0088` (`p:1260`) | `alias-clause` with leading `#` must follow integer-literal grammar. | Ordinal-alias parse rule and declaration normalization. |
| `CONF-discovered-ms-vbal-250520-f945507e-0090` (`p:1261`) | Non-ordinal alias syntax is implementation-defined. | Symbol alias acceptance with explicit implementation-defined note. |
| `CONF-discovered-ms-vbal-250520-f945507e-0091` (`p:1262`) | Additional restrictions on external procedure declarations are allowed. | Profile/policy-dependent restriction model (compile-time/runtime). |
| `CONF-discovered-ms-vbal-250520-f945507e-0092` (`p:1263`) | Additional restrictions for non-`PtrSafe` declarations are allowed. | `PtrSafe` governance rule hook. |
| `CONF-discovered-ms-vbal-250520-f945507e-0093` (`p:1266`) | Alias is used in an implementation-defined way to select procedure. | Deterministic symbol selection algorithm must be documented. |
| `SPEC-discovered-ms-vbal-250520-f945507e-01617` (`p:1259`) | Case-sensitive vs case-insensitive external name interpretation is implementation-defined. | Per-profile symbol-name matching policy must be explicit. |
| `SPEC-discovered-ms-vbal-250520-f945507e-01624..01627` | Library string and alias/proc-name selection are implementation-defined. | Loader/selector strategy must be documented and testable. |

### 3.2 MS-OAUT anchors (Automation marshaling constraints)

| Anchor ID | Summary | Role in OxVba HAL contract |
|---|---|---|
| `CONF-discovered-ms-oaut-240423-b76f9b41-0010` | `VT_BYREF` must be OR-ed with another variant type. | ByRef variant validation rule. |
| `CONF-discovered-ms-oaut-240423-b76f9b41-0011` | `VT_EMPTY`/`VT_NULL` must not be combined with `VT_BYREF`. | Invalid discriminant rejection. |
| `...-0015`, `...-0016` | Certain variant flags require no data field. | Variant payload-shape checks. |
| `...-0023..0029` | Type compatibility constraints for `BSTR`, `IDispatch*`, `HRESULT`, `VARIANT_BOOL`, `VARIANT`, `IUnknown*`. | Boundary marshaling type matrix. |
| `...-0042`, `...-0050..0052` | `SAFEARRAY` typing and byref flag constraints. | Array marshaling legality checks. |
| `...-0058..0060` | SAFEARRAY element typing for `BSTR`, `IUnknown*`, `IDispatch*`. | Array element-type conformance checks. |
| `CONF-discovered-ms-oaut-210625-4fcc3347-0080..0084` | `Invoke` output behavior (`zeroVarResult`, `zeroExcepInfo`, `ArgErr`). | COM dispatch bridge obligations when COM lane is active. |

### 3.3 MS-DTYP anchors (string/pointer ABI support)

| Anchor ID | Summary | Role in OxVba HAL contract |
|---|---|---|
| `CONF-discovered-ms-dtyp-241119-518a70cb-0002`, `...-0007`, `...-0008` | `LPCWSTR`/`LPWSTR` pointer model and optional null termination. | UTF-16 pointer-string assumptions. |
| `...-0003` | `LPSTR` pointer-string model. | 8-bit string pointer assumptions. |
| `...-0004`, `...-0009` | Pointer strings require explicit string semantics or explicit length. | Marshaling metadata requirement for pointer-based strings. |
| `...-0005` | Character format must be protocol-specified. | OxVba ABI lane must define encoding explicitly. |

## 4. HAL Placement and Layered Contract

`Declare` is split into three explicit layers:

1. Compile-time declaration contract:
- parser + resolver normalize declaration metadata,
- policy/profile restrictions can reject declarations before execution.

2. Runtime selection contract:
- VM lowers external call to HAL dynamic-link operation,
- symbol selection algorithm is deterministic and diagnostics-stable.

3. Marshaling contract:
- argument/result mapping obeys lane-specific type matrix and failure rules,
- invalid shapes fail deterministically (no undefined behavior, no silent coercion).

Current trait anchor:
- `DynamicLinkHal::invoke_symbol(symbol_token, arg_token) -> token`

V1 keeps this narrow trait, with marshaling richness represented in policy/capability and contract clauses.

## 5. Capability/Profile Contract

| Profile | `DynamicLinking` baseline | COM/Automation relation | Contract class |
|---|---|---|---|
| `windows` | supported (subset) | COM capability may be supported separately | `host-backed` or deterministic projection |
| `linux` | supported (subset) | COM unsupported | `host-backed` or deterministic projection |
| `macos` | unsupported in v1 | COM unsupported | deterministic unsupported |
| `wasm` | unsupported in v1 | COM unsupported | deterministic unsupported |
| `null` | unsupported in v1 | COM unsupported | deterministic unsupported floor |

Policy mode behavior:
- `CompileTime`: reject unsupported or disallowed paths before run.
- `Runtime`: allow execution; fail deterministically at host boundary.

## 6. Marshaling Classes

### 6.1 M0 (implemented subset)

Current lane in `HAL_DECLARE_EXECUTION_IMPLEMENTATION_V2.md`:
- deterministic symbol-token projection and bounded host-backed known-symbol mapping,
- scalar token in/out path,
- deterministic adapter-fault for unresolved symbols,
- declaration surface restricted to:
  - `Declare PtrSafe`,
  - max one argument,
  - `ByVal ... As Long` parameter type only,
  - `Function ... As Long` return type only.

### 6.2 M1 (specified target: Automation-compatible lane)

Planned obligations:
- `VARIANT` discriminant legality checks (`VT_BYREF` matrix, null/empty exclusions),
- `SAFEARRAY` element-type constraints for in-scope Automation types,
- deterministic error mapping when bridge cannot satisfy OAUT shape constraints.

### 6.3 M2 (specified target: native C ABI lane)

Planned obligations:
- calling convention and width rules explicit per profile/runtime-class,
- pointer-string semantics require explicit encoding + length/termination model,
- unsupported declaration shapes rejected deterministically.

## 7. Failure Contract

Required stable behavior for all lanes:
- unsupported capability: `HAL-E-CAP-UNAVAILABLE`,
- policy denial: `HAL-E-POLICY-DENIED`,
- selection/marshaling/adapter failure: `HAL-E-ADAPTER-FAULT`.

Diagnostic payload requirements:
- stable code,
- capability,
- operation,
- profile,
- deterministic reason text family.

VM surface:
- routed through existing host-error path with `On Error` semantics preserved.

## 8. Ambiguities and Inconsistencies (Tracked)

1. Alias and library-string procedure selection is explicitly implementation-defined by MS-VBAL.
2. External name case-sensitivity is implementation-defined.
3. `PtrSafe` restrictions are permissive (`MAY`) and therefore profile-policy dependent.
4. Extraction quality is mixed (`candidate` rows, OCR artifacts); final parity claims require canonical-source pass and empirical validation.
5. Cross-platform ABI behavior is inherently non-uniform; compatibility claims must be scoped by profile/runtime-class.

Tracking locations:
- `docs/evidence/hal/HAL_UNCERTAINTY_REGISTER.md`
- `docs/evidence/hal/HAL_IMPLEMENTATION_DEFINED.md`
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.md`

## 9. Clause Mapping

Primary clause catalog section:
- `docs/spec/HAL_CONTRACT_CLAUSE_CATALOG_V1.md` (`HAL-DYN-*`)

Conformance planning companion:
- `docs/spec/HAL_DECLARE_MARSHAL_CONFORMANCE_V1.md`

## 10. Open Decisions

1. Whether `DynamicLinkHal` should split into explicit bind/prepare/invoke phases.
2. Profile-level default calling-convention policy when declaration omits explicit convention.
3. Canonical typed boundary for non-token marshaling lanes and how it coexists with current `ValueToken = i32`.
