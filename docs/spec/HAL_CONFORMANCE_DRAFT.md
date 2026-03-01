# HAL Conformance Draft (Early Stage)

Status: `design-draft`  
Date: 2026-03-01

## 1. Intent

This draft defines how OxVba HAL conformance is evaluated without requiring all profiles/capabilities to be complete from day one.

It separates:
- whether a capability exists,
- how mature/reliable it is,
- whether declared behavior matches observed behavior.

## 2. Conformance Axes

1. Interface conformance:
- Required trait/API contract exists and is version-compatible.

2. Capability declaration conformance:
- Descriptor truthfully reports support and maturity.

3. Behavioral conformance:
- For supported capabilities, required semantics/tests pass.
- For unsupported capabilities, deterministic failure semantics pass.

4. Policy conformance:
- Host policy controls are enforced consistently.

5. Evidence conformance:
- Required artifacts/logs are emitted and discoverable.

## 3. Conformance Levels

`L0` (contract floor):
- compiles against HAL interfaces,
- descriptor emits all capability entries,
- unsupported pathways deterministic.

`L1` (behavioral baseline):
- deterministic domain tests pass for every declared supported capability,
- policy-denied and unavailable paths pass for every domain.

`L2` (reference-aligned):
- profile-specific reference checks pass where oracle exists (for example Office on Windows for relevant behavior classes),
- drift watchlist is tracked and triaged.

`L3` (production confidence target):
- sustained pass history,
- reliability/performance envelopes documented,
- maturity levels justified by evidence, not declaration only.

Not every profile must reach the same level simultaneously.

## 4. Profile Expectations (Initial)

- `windows`: target `L2` first for COM/interaction-sensitive surfaces.
- `linux`: target `L1` across supported domains; selective `L2` where reference behavior is platform-neutral.
- `macos`: target `L0/L1` initially, uplift as coverage expands.
- `wasm`: target `L0/L1` with constrained capabilities and strict policy gates.
- `null`: always expected to pass deterministic unsupported behavior and selected pure deterministic pathways; serves as baseline oracle.

## 5. Test Strategy Sketch

### 5.1 Contract tests
- Ensure every adapter implements required interfaces and descriptor completeness.

### 5.2 Deterministic unsupported tests
- Assert unsupported operations produce stable error families and no hidden side effects.

### 5.3 Policy enforcement tests
- For each gated operation, verify allow/deny/virtualization modes.

### 5.4 Behavioral tests (supported capability only)
- Domain-specific semantic assertions (e.g. `MsgBox` return mapping, `DoEvents` queue effects, COM activation behavior where supported).

### 5.5 Reference/differential tests
- Compare against canonical runtime behavior where oracle exists.
- Track unresolved differences in divergence docs.

### 5.6 Harness layering (pre-engine and in-engine)
- Pre-engine HAL probe suite:
  - validates adapter contract, capability descriptor integrity, and deterministic unsupported/policy behavior independent of VBA execution.
- In-engine integration suite:
  - exercises host-sensitive VBA/library surfaces (`MsgBox`, `InputBox`, `DoEvents`, `CreateObject`, `Shell`, file operations) through the runtime.
- Both lanes are required for maturity promotion.

## 6. Required Evidence Artifacts (Draft)

Per profile run:
- HAL descriptor snapshot (`json`/`md`).
- Capability matrix with maturity and support.
- Policy-mode test outputs.
- Domain conformance summary.
- Deferred issues list for non-blocking failures.

These should live under `docs/evidence/hal/` in later implementation phases.

## 7. Non-Blocking Failure Policy (Current Stage)

During initial rollout:
- HAL conformance failures are non-blocking for overall ladder execution unless they imply unsoundness or data corruption risk.
- Failures are logged, triaged, and attached to deferred-gate/backlog artifacts.
- Capability maturity may be downgraded when failures persist.

## 8. Relationship to Language Conformance

Language conformance and HAL conformance are orthogonal but connected:
- Core language tests should pass on all profiles using pure/runtime-only pathways.
- Host-sensitive language/library features are evaluated against HAL capability declarations.

No profile may claim full host-sensitive conformance without matching HAL evidence.

## 9. Open Questions

1. Should maturity promotion require minimum burn-in duration or only pass counts?
2. How should flaky host-dependent behavior affect maturity level assignment?
3. Which host-sensitive behaviors need strict oracle matching versus bounded compatibility envelopes?
4. Should a profile be allowed to publish `L2` if one domain remains `L1`?
