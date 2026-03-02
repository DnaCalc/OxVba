# HAL Formalization Program

Status: `working-draft`  
Date: 2026-03-02  
Priority order source: `CHARTER.md` (`Robustness > Compatibility > Performance`)

## 1. Program Statement

Formal HAL program driven by CHARTER priorities (robustness/compatibility first).

- [x] Audit current HAL docs/code against formal contract needs and identify gaps by capability/domain.
- [x] Author phased HAL formal spec set (phase ladder, contract clauses, pre/post-conditions, failure semantics, profile/null behavior, uncertainty/implementation-defined registries).
- [ ] Implement executable contract checks (conformance and property tests) mapped to spec clauses.
- [ ] Refine adapter implementations to satisfy the tightened contract and rerun evidence suite.
- [ ] Update project references/logs and summarize resulting HAL operating envelope and open uncertainties.

This program is intentionally specification-first so runtime integration never depends on ambiguous host behavior.

## 2. H1-H3 Execution Tracks

### H1. Contract-Level Formalization (trait semantics and invariants)

Objective:
- Define HAL contracts with precise preconditions/postconditions, invariants, and failure classes before broad implementation changes.

Scope:
- Trait-by-trait contract clauses for:
  - `UiInteractionHal`
  - `EventPumpHal`
  - `FileSystemHal`
  - `ProcessEnvHal`
  - `ComHal`
  - `TimeLocaleHal`
  - `DynamicLinkHal`
  - `DiagnosticsHal`
- Capability model invariants (`supported`, `maturity`, profile-level expectations).
- Policy invariants (`allow_*`, deterministic mode, unsupported-feature mode).
- Null profile deterministic unsupported contract as a reference floor.

Outputs:
- Normative contract tables and clause IDs.
- `uncertainty` registry for unresolved contract shape questions.
- `implementation-defined` registry for explicit non-VBA guarantees at HAL boundary.

### H2. Native Profile Realization (Windows-first while preserving cross-profile contract)

Objective:
- Implement concrete host behavior where feasible without violating H1 clauses.

Scope:
- Windows:
  - replace deterministic placeholders with real host operations where contract model is mature.
- Linux/macOS/WASM/null:
  - preserve deterministic failure/partial behavior in strict conformance with clauses.

Outputs:
- Per-profile conformance matrices with clause coverage.
- Divergence/uncertainty updates where host realities force contract refinement.

### H3. ABI and External Adapter Boundary (optional, versioned)

Objective:
- Add a versioned external boundary once Rust trait contracts are stable.

Scope:
- Define `hal_abi_v1` (`repr(C)` vtables, opaque context handles, stable error surface).
- Keep Rust-native and C-ABI adapters behaviorally equivalent under the same conformance suite.

Outputs:
- ABI spec, compatibility/version policy, and adapter bridge tests.

## 3. Formal Contract Shape Requirements

Each HAL operation clause should specify:
- Preconditions:
  - capability support conditions,
  - policy requirements,
  - argument domain constraints.
- Postconditions:
  - result shape and determinism guarantees,
  - state transition guarantees (for stateful domains like file handles).
- Failure semantics:
  - required `HAL-E-*` family,
  - side-effect guarantees on failure (`no hidden mutation` where applicable).
- Profile notes:
  - support/maturity status and profile-specific constraints.
- Verification mapping:
  - conformance probe(s),
  - property tests and/or integration checks.

## 4. Registry Requirements

To keep contract drift explicit and reviewable:

1. HAL Uncertainty Registry:
- tracks unresolved contract questions, rationale, decision owner, and expected resolution phase.

2. HAL Implementation-Defined Registry:
- tracks explicit implementation-defined behaviors that are not yet constrained by VBA spec text.

3. HAL Contract Change Log:
- each clause change records compatibility impact and required conformance updates.

## 5. Clause-to-Test Discipline

Every finalized clause must have at least one executable check:
- pre-engine conformance probe and/or property test,
- in-engine integration test where runtime-visible,
- profile-specific assertions where relevant.

No clause should remain undocumented in terms of verification mapping.

## 6. Immediate Next Slice (Program Bootstrap)

1. Create clause catalog v1 for all current traits and policy/capability primitives.
2. Introduce uncertainty + implementation-defined registries under `docs/evidence/hal/`.
3. Expand conformance harness to report clause IDs and pass/fail per profile/lane.
4. Use this as the gate for subsequent Windows-native behavior implementation work.
