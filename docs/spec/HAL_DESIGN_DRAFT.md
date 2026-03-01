# HAL Design Draft (Early Stage)

Status: `design-draft`  
Date: 2026-03-01  
Scope owner: OxVba core/runtime design

## 1. Purpose

This draft defines the first structured design frame for an OxVba Host Abstraction Layer (HAL).

Goal:
- Keep VBA language/runtime semantics portable and deterministic.
- Isolate host/platform effects behind explicit interfaces.
- Make unsupported host features fail in predictable, testable ways.
- Enable test harness virtualization of interactive/environment-dependent features.

This is intentionally not a final normative spec. It is a design-run scaffold that prepares a formal spec and implementation sequence.

## 2. Why HAL Exists

OxVba must support:
- Windows (full COM and native host integration paths).
- Linux and macOS (portable host pathways; COM activation/dispatch is explicitly unsupported in current baseline).
- WASM sandbox (highly constrained host surface).
- Null HAL (deterministic no-host baseline and failure-shape oracle).

Without HAL, host-specific behavior leaks into language/runtime code paths and blocks rigorous conformance and reproducibility.

## 3. Design Principles

1. Compatibility-first semantics:
Language-level behavior matches VBA specification where applicable; host differences are explicit and capability-gated.

2. Deterministic degradation:
If a host capability is absent, behavior is deterministic and diagnostically explicit.

3. Capability + maturity (not just capability):
Each service is represented by:
- support status (`absent`, `available`)
- maturity level (`stub`, `experimental`, `provisional`, `stable`)

4. Policy-controlled interaction:
Interactive and side-effecting services (`MsgBox`, `InputBox`, `Shell`, `CreateObject`, filesystem, env/process) are mediated by host policy and can be virtualized for automated runs.

5. Separate platform profile from language core:
Core language/runtime should compile and run independently of platform-specific adapters.

6. Evidence-linked evolution:
Changes in HAL contracts require updates to conformance obligations and evidence artifacts.

## 4. HAL Profile Targets (v1 Design Baseline)

Planned profile set from the start:
- `windows`
- `linux`
- `macos`
- `wasm`
- `null`

Rationale for Linux/macOS split:
- Shared POSIX assumptions are useful but incomplete for host APIs, packaging, sandboxing, eventing, and system integration details.
- A split avoids overpromising one "Unix" behavior class.

## 5. Service Domains in Scope

1. `ui_interaction`:
- `MsgBox`, `InputBox`
- Optional non-interactive virtualization mode with scripted responses

2. `event_pump`:
- `DoEvents`
- Event queue integration contract (host-driven + runtime-driven)

3. `filesystem_io`:
- File open/read/write/seek/close abstractions used by runtime/library surface

4. `process_env`:
- `Shell`, environment variable access, current directory and path-related behavior

5. `com_activation_dispatch`:
- `CreateObject`, dispatch invocation and late-bound pathways
- Windows: real COM
- Non-Windows: deterministic unsupported result in current baseline

6. `time_locale`:
- Date/time, locale-sensitive formatting/parsing hooks where host influence exists

7. `dynamic_linking`:
- Declare/foreign call host loading surface (capability-gated, policy-controlled)

8. `diagnostics_telemetry`:
- Structured runtime host diagnostics and capability disclosures

## 6. Early Architecture Direction

Short term:
- Rust trait contracts for HAL interfaces.
- Runtime holds a `HostServices` object with domain-specific subtraits.
- Capability/maturity descriptor returned at startup and embedded in diagnostics/evidence.

Later optional extension:
- Stable external ABI boundary (C ABI or similar) for out-of-process or foreign-language adapters.

The C ABI path is explicitly deferred until Rust-level contracts are mature and evidenced.

## 7. Virtualization Requirements (Critical)

HAL must support non-UI deterministic execution for automation:
- `MsgBox` virtual mode:
  - consume pre-seeded responses
  - record presented prompts/options
- `InputBox` virtual mode:
  - consume deterministic script values
  - support cancel/empty distinctions
- `DoEvents` virtual mode:
  - deterministic queue model for tests

This is required for reliable conformance and CI-style reproducible runs.

## 8. Conformance Philosophy (Preview)

HAL conformance will be multi-dimensional:
- Interface conformance: contract completeness and shape.
- Behavioral conformance: required semantics for supported capabilities.
- Maturity conformance: declared maturity must match observed behavior quality.
- Evidence conformance: required test/probe artifacts published per profile.

Detailed structure is specified in [`HAL_CONFORMANCE_DRAFT.md`](HAL_CONFORMANCE_DRAFT.md).

## 9. Open Design Questions

1. What are the acceptance criteria for eventually enabling non-Windows COM-like pathways (if ever), versus retaining explicit unsupported behavior?
2. Should `DoEvents` have a strict minimum scheduling contract across all profiles?
3. What is the minimum viable WASM host surface for language/library parity claims?
4. Which host-sensitive built-ins can be guaranteed deterministic on all profiles?
5. What maturity promotion criteria are mandatory for `experimental -> provisional -> stable`?

## 10. Initial Roadmap for Spec Run

Stage A: freeze terminology and service-domain boundaries (this draft set).  
Stage B: define interface signatures + capability descriptors (`HAL_INTERFACE_DRAFT.md`).  
Stage C: define conformance classes + gates (`HAL_CONFORMANCE_DRAFT.md`).  
Stage D: implement `null` HAL as conformance floor oracle.  
Stage E: bring up profile adapters and publish per-profile evidence matrices.

## 11. Spec Source Anchors

Primary VBA/Automation source families to map in later normative text:
- MS-VBAL (VBA language semantics)
- MS-OAUT (Automation/VARIANT/IDispatch/SAFEARRAY)
- MS-COM (COM model/protocol)
- MS-VBA and Office VBA documentation for runtime/library behavior and host-interaction surfaces

This draft establishes direction; the follow-up normative mapping table is deferred to the next spec stage.

## 12. Early Decisions To Lock Before Implementation

1. Capability identifier stability:
- Define canonical capability IDs now to prevent evidence churn when adapters are implemented.

2. HAL descriptor versioning:
- Decide versioning policy (`hal_contract_version`, profile adapter version) and compatibility rules.

3. Policy defaults:
- Decide secure-by-default behavior for interaction/process/dynamic-link permissions in embedded-host and CI modes.

4. Queue/event contract for `DoEvents`:
- Decide minimum cross-profile guarantees (for example whether a no-queue host must still provide a deterministic no-op result shape).

5. Conformance gate ownership:
- Decide whether HAL conformance gates are separate from language gates or merged as a required sub-gate at selected ladders.

6. Maturity downgrade rules:
- Define explicit triggers that force capability maturity rollback when regressions are observed.
