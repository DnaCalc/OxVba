# HAL Declare + Marshaling Conformance Plan V1

Status: `working-draft`  
Date: 2026-03-02  
Scope: spec/contracts/conformance planning for `Declare` and boundary marshaling

## 1. Purpose

Define executable conformance lanes for external declarations and marshaling with explicit clause mapping, while separating:
- currently implemented deterministic subset checks,
- planned native/automation compatibility checks,
- deferred empirical-oracle checks.

## 2. Source-Crosswalk Baseline

Primary spec anchors are tracked in:
- `HAL_DECLARE_ABI_SPEC_V1.md`
- `HAL_SPEC_CROSSWALK.md`

Anchor families:
- MS-VBAL: declaration grammar + implementation-defined selection/restriction points.
- MS-OAUT: `VARIANT`/`SAFEARRAY`/`IDispatch::Invoke` marshaling rules.
- MS-DTYP: pointer string/encoding metadata requirements.

## 3. Clause Coverage Targets

| Clause ID | Topic | Verification lane | Current state |
|---|---|---|---|
| `HAL-DYN-001` | DynamicLink capability + policy gating floor | existing HAL conformance (`dynlink.invoke_symbol`) | implemented-verified |
| `HAL-DYN-002` | Alias grammar/normalization (`#ordinal` vs symbolic alias) | compiler/resolver tests | specified-pending |
| `HAL-DYN-003` | Implementation-defined name-selection policy surfaced explicitly | descriptor/doc + host diagnostics checks | specified-pending |
| `HAL-DYN-004` | `PtrSafe`/declaration-shape policy restrictions | compile-time policy tests | specified-pending |
| `HAL-DYN-005` | `VARIANT` byref discriminant legality rules | marshaling unit/property tests | specified-pending |
| `HAL-DYN-006` | `SAFEARRAY` element-type legality matrix | marshaling unit/property tests | specified-pending |
| `HAL-DYN-007` | pointer-string metadata/encoding rules (`LPSTR`/`LPWSTR`) | marshaling shape tests + diagnostics assertions | specified-pending |
| `HAL-DYN-008` | `IDispatch::Invoke` out-param obligations (`VarResult`/`ExcepInfo`/`ArgErr`) | COM bridge integration tests (Windows lane) | specified-pending |
| `HAL-DYN-009` | Dynamic-link marshaling failure determinism and stable diagnostics | host/VM error-routing tests | specified-pending |
| `HAL-DYN-010` | Unsupported declaration forms fail deterministically by mode | compile-time/runtime dual-mode tests | specified-pending |

## 4. Lanes

### 4.1 Lane A: Declaration/Resolver static conformance

Targets:
- parser acceptance/rejection around alias/ordinal rules,
- declaration metadata normalization (`Lib`, `Alias`, procedure id),
- policy-gated declaration restrictions (compile-time mode).

Evidence:
- compiler tests in `oxvba-compiler` + host preflight tests in `oxvba-host`.

### 4.2 Lane B: VM/HAL dynamic-link subset conformance

Targets:
- deterministic lowering to host dynamic-link instruction,
- runtime policy denial/capability-unavailable shape,
- deterministic adapter-fault on unresolved symbol path.

Evidence:
- existing host + VM tests for `IntrinsicInvokeSymbolHost`.

### 4.3 Lane C: Marshaling contract conformance (planned)

Targets:
- `VARIANT` and `SAFEARRAY` rule checks derived from MS-OAUT anchors,
- pointer-string metadata/encoding checks derived from MS-DTYP anchors,
- deterministic failure for invalid shapes.

Evidence target:
- dedicated marshaling test module and property checks.

### 4.4 Lane D: Platform/profile integration conformance

Targets:
- Windows/Linux profile policy and diagnostics parity for supported subset,
- deterministic unsupported behavior for `macos`/`wasm`/`null`.

Evidence:
- `scripts/run-hal-conformance.ps1`,
- wasm lane `scripts/run-hal-conformance-wasm32.ps1`,
- host-specific integration reports under `docs/evidence/hal`.

### 4.5 Lane E: Deferred empirical/oracle lane

Targets:
- implementation-defined behavior parity probes against real VBA host behavior.

Evidence location:
- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.md`

Policy:
- non-blocking at this stage, but tracked with explicit divergence IDs.

## 5. Inconsistency/Ambiguity Handling

When sources are permissive (`MAY`) or implementation-defined:
1. Record decision in `HAL_IMPLEMENTATION_DEFINED.md`.
2. Record uncertainty in `HAL_UNCERTAINTY_REGISTER.md` when unresolved.
3. Mark clause as `specified-pending` until executable checks are in place.
4. Add deferred-oracle item if real-host behavior is needed for compatibility claim.

## 6. Immediate Execution Priorities

1. Implement Lane A tests for alias grammar/normalization and policy restrictions.
2. Introduce marshaling-shape contract tests for `HAL-DYN-005..007`.
3. Extend host/VM diagnostics tests for dynamic-link failure taxonomy (`HAL-DYN-009`).

## 7. Progress Snapshot (Current)

- Lane A implemented in compiler/host subset:
  - `Declare PtrSafe` enforced in v1 subset,
  - alias canonicalization and ordinal validation implemented,
  - unsupported declaration forms (`ByRef`, `Optional`, `ParamArray`, multi-arg, non-`Long` parameter/return types) rejected with deterministic diagnostics.
- Clause status updates:
  - `HAL-DYN-002`, `HAL-DYN-004` moved to implemented-verified,
  - `HAL-DYN-003`, `HAL-DYN-009`, `HAL-DYN-010` moved to implemented-partial.
