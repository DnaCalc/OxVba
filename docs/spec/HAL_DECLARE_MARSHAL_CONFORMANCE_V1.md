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
| `HAL-DYN-002` | Alias grammar/normalization (`#ordinal` vs symbolic alias) | compiler/resolver tests | implemented-verified |
| `HAL-DYN-003` | Implementation-defined name-selection policy surfaced explicitly | descriptor/doc + host diagnostics checks | implemented-partial |
| `HAL-DYN-004` | `PtrSafe`/declaration-shape policy restrictions | compile-time policy tests | implemented-verified |
| `HAL-DYN-005` | `VARIANT` byref discriminant legality rules | declaration-subset restriction tests + future marshaling unit/property tests | implemented-partial |
| `HAL-DYN-006` | `SAFEARRAY` element-type legality matrix | declaration-subset restriction tests + future marshaling unit/property tests | implemented-partial |
| `HAL-DYN-007` | pointer-string metadata/encoding rules (`LPSTR`/`LPWSTR`) | declaration-subset restriction tests + descriptor rejection assertions | implemented-partial |
| `HAL-DYN-008` | `IDispatch::Invoke` out-param obligations (`VarResult`/`ExcepInfo`/`ArgErr`) | COM bridge integration tests (Windows lane) | specified-pending |
| `HAL-DYN-009` | Dynamic-link marshaling failure determinism and stable diagnostics | host/VM error-routing tests | implemented-partial |
| `HAL-DYN-010` | Unsupported declaration forms fail deterministically by mode | compile-time/runtime dual-mode tests | implemented-partial |
| `HAL-DYN-011..013` | Descriptor model + metadata + descriptor-driven routing | compiler + VM + HAL conformance (`dynlink.invoke_descriptor`) | implemented-partial |
| `HAL-DYN-014..015` | compile-time/runtime mode contract parity over descriptor path | host preflight/runtime tests + conformance | implemented-partial |
| `HAL-DYN-016..017` | windows/linux host-backed dynamic-link contract probes | HAL conformance (`evaluate_dynlink_contract_paths`) | implemented-partial |
| `HAL-DYN-018..019` | pointer-string and byref-writeback lanes | deterministic unsupported-lane rejection checks (`evaluate_dynlink_contract_paths`) | implemented-partial |
| `HAL-DYN-020` | lane selection determinism (`M0/M1/M2`) | descriptor metadata + adapter lane checks | implemented-partial |

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
3. Mark clause as `implemented-partial` if executable checks exist but lane breadth is still intentionally constrained.
4. Add deferred-oracle item if real-host behavior is needed for compatibility claim.

## 6. Immediate Execution Priorities

1. Expand full marshaling-shape legality tests for `HAL-DYN-005..008`, with priority on `HAL-DYN-008` and full Automation-matrix closure for `HAL-DYN-005..007`.
2. Harden descriptor-policy preflight checks for `HAL-DYN-014` and richer lane metadata validation.
3. Progress host-backed loader coverage beyond bounded known-symbol subset.

## 7. Progress Snapshot (Current)

- Lane A implemented in compiler/host subset:
  - `Declare PtrSafe` enforced in v1 subset,
  - alias canonicalization and ordinal validation implemented,
  - unsupported declaration forms (`ByRef`, `Optional`, `ParamArray`, multi-arg, non-`Long` parameter/return types) rejected with deterministic diagnostics.
- Clause status updates:
  - `HAL-DYN-002`, `HAL-DYN-004` moved to implemented-verified,
  - `HAL-DYN-003`, `HAL-DYN-005..007`, `HAL-DYN-009`, `HAL-DYN-010` moved to implemented-partial,
  - descriptor-lane clauses `HAL-DYN-011..020` now have executable conformance hooks and are tracked as implemented-partial pending richer marshaling/ABI breadth,
  - `HAL-DYN-018..019` include explicit deterministic unsupported-lane rejection checks in conformance.
