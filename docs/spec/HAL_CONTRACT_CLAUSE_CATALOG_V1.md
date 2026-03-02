# HAL Contract Clause Catalog v1

Status: `working-draft`  
Date: 2026-03-02  
Applies to code baseline: `crates/oxvba-hal` + HAL integration in `oxvba-host`/`oxvba-vm`

## 1. Purpose

This catalog defines explicit HAL contract clauses with stable IDs so behavior changes can be reviewed against:
- robustness constraints,
- compatibility impact,
- conformance evidence.

Machine-readable companion catalog:
- [`HAL_CONTRACT_CLAUSE_CATALOG_V1.csv`](HAL_CONTRACT_CLAUSE_CATALOG_V1.csv)

Each clause includes:
- scope,
- preconditions,
- postconditions,
- failure obligations,
- current verification mapping.

## 2. Clause Status Vocabulary

- `implemented-verified`: implemented and mapped to executable checks.
- `implemented-partial`: implemented but only partially verified.
- `specified-pending`: specified but not yet implemented and/or not verified.

## 3. Global Clauses

| Clause ID | Clause | Status | Verification |
|---|---|---|---|
| `HAL-GEN-001` | HAL operations must terminate with either success or structured `HalError`; no silent unsupported no-op behavior is permitted. | implemented-verified | `crates/oxvba-hal/src/conformance.rs` |
| `HAL-GEN-002` | `HalDescriptor` must include a single entry for every capability in `ALL_CAPABILITIES`. | implemented-verified | `validate_descriptor_shape` in `conformance.rs` |
| `HAL-GEN-003` | Unsupported capability behavior must return `HalErrorKind::CapabilityUnavailable` with stable code `HAL-E-CAP-UNAVAILABLE`. | implemented-verified | `run_conformance` unsupported-path assertions |
| `HAL-GEN-004` | Policy-denied behavior must return `HalErrorKind::PolicyDenied` with stable code `HAL-E-POLICY-DENIED`. | implemented-verified | `run_conformance` policy-gated checks, host tests |
| `HAL-GEN-005` | `HostPolicy.unsupported_feature_mode = CompileTime` requires host preflight rejection of host-sensitive bytecode when capability/policy constraints fail. | implemented-verified | `hal_compile_time_mode_*` tests in `oxvba-host` |
| `HAL-GEN-006` | `HostPolicy.unsupported_feature_mode = Runtime` permits execution; failure must surface at runtime through deterministic host error propagation. | implemented-verified | `hal_runtime_mode_surfaces_host_error_for_unsupported_linux_com_intrinsics` |
| `HAL-GEN-007` | Profile-level COM support baseline: Windows supported; non-Windows unsupported. | implemented-verified | `windows_declares_com_supported_only_on_windows` |
| `HAL-GEN-008` | Conformance clause references must resolve to known IDs in the machine-readable clause catalog (drift guard). | implemented-verified | `validate_clause_reference_integrity` in `conformance.rs` |

## 4. Descriptor and Capability Clauses

| Clause ID | Clause | Status | Verification |
|---|---|---|---|
| `HAL-DES-001` | `descriptor.contract_version` must be non-empty. | implemented-verified | `validate_descriptor_shape` |
| `HAL-DES-002` | `descriptor.adapter_version` must be non-empty. | implemented-verified | `validate_descriptor_shape` |
| `HAL-DES-003` | Duplicate capability descriptors are invalid. | implemented-verified | `validate_descriptor_shape` |
| `HAL-DES-004` | `supported = false` implies all operations in that capability fail with `CapabilityUnavailable` unless compile-time gate intercepts earlier. | implemented-verified | `run_conformance` + host compile-time gate tests |
| `HAL-DES-005` | Capability `maturity` is metadata only at v1; it must not weaken failure determinism rules. | implemented-verified | `maturity_does_not_affect_policy_denial_shape` |

## 5. Error Contract Clauses

| Clause ID | Clause | Status | Verification |
|---|---|---|---|
| `HAL-ERR-001` | Stable codes must be used: `HAL-E-CAP-UNAVAILABLE`, `HAL-E-POLICY-DENIED`, `HAL-E-ADAPTER-FAULT`, `HAL-E-UNSUPPORTED-PROFILE`. | implemented-verified | `expected_stable_code_for_kind` + conformance probe validation |
| `HAL-ERR-002` | Error payload must include profile, capability, operation, and message. | implemented-verified | conformance payload-shape checks in `run_conformance`; `HalError` schema in `error.rs` |
| `HAL-ERR-003` | VM host error routing must preserve VBA error-control behavior (`On Error` paths) and produce deterministic runtime diagnostics when unhandled. | implemented-verified | `hal_runtime_mode_routes_host_error_through_on_error_resume_next`; `hal_compile_time_mode_rejects_even_with_on_error_resume_next` |

## 5.5 Governance Clauses (Exploratory Non-Blocking)

| Clause ID | Clause | Status | Verification |
|---|---|---|---|
| `HAL-GOV-001` | Supported capabilities should not remain at `Stub` maturity; if they do, emit deterministic governance notice entries. | implemented-verified | `evaluate_maturity_governance` + `governance_rules_are_executable_and_non_blocking` |
| `HAL-GOV-002` | Unsupported capabilities should remain `Stable` maturity metadata in v1; deviations emit deterministic governance notice entries. | implemented-verified | `evaluate_maturity_governance` + `governance_rules_are_executable_and_non_blocking` |

## 6. Domain Clauses

### 6.1 `UiInteractionHal`

| Clause ID | Operation | Preconditions | Postconditions | Failure obligations | Status | Verification |
|---|---|---|---|---|---|---|
| `HAL-UI-001` | `msg_box(prompt, style)` | capability supported; interaction allowed by policy | returns deterministic `ValueToken` response | unsupported -> `CapabilityUnavailable`; denied -> `PolicyDenied` | implemented-verified | conformance `ui.msg_box` probe; `ui_msg_box_enforces_policy_and_capability_failures` |
| `HAL-UI-002` | `input_box(prompt, default)` | capability supported; interaction allowed by policy | returns deterministic response token by virtualization mode | unsupported/denied failure as above | implemented-verified | `ui_virtualization_modes_follow_contract`; `ui_fail_on_prompt_returns_policy_denied` |
| `HAL-UI-003` | Virtualization mode controls result shape (`ScriptedResponses`, `Disabled`, `FailOnPrompt`) | valid policy | deterministic branch-specific outcome | `FailOnPrompt` returns policy denial | implemented-verified | `ui_virtualization_modes_follow_contract`; `ui_fail_on_prompt_returns_policy_denied` |

### 6.2 `EventPumpHal`

| Clause ID | Operation | Preconditions | Postconditions | Failure obligations | Status | Verification |
|---|---|---|---|---|---|---|
| `HAL-EVT-001` | `do_events()` | capability supported | deterministic token return | unsupported -> `CapabilityUnavailable` | implemented-verified | conformance `events.do_events`; `event_pump_supported_and_unsupported_paths` |
| `HAL-EVT-002` | v1 does not define external queue fairness or scheduling guarantees. | none | deterministic local contract only | n/a | specified-pending | tracked in uncertainty registry |

### 6.3 `FileSystemHal`

| Clause ID | Operation | Preconditions | Postconditions | Failure obligations | Status | Verification |
|---|---|---|---|---|---|---|
| `HAL-FS-001` | `open(path, mode)` | capability supported; mutation allowed when `mode != 0` | allocates deterministic handle in supported range; initializes file state | unsupported/policy-denied/adapter-fault as applicable | implemented-verified | `file_open_seek_eof_lof_close_roundtrip`; `file_open_denied_has_no_state_side_effects`; `prop_free_file_low_range_tracks_open_count` |
| `HAL-FS-002` | `close(handle)` | valid open handle | handle removed from state; returns success token | invalid handle -> `AdapterFault` | implemented-verified | `file_open_seek_eof_lof_close_roundtrip` |
| `HAL-FS-003` | `seek(handle, position)` | valid handle; non-negative position | updates position; optionally extends logical length in mutation mode | invalid handle or negative position -> `AdapterFault` | implemented-verified | `file_open_seek_eof_lof_close_roundtrip`; `seek_negative_returns_adapter_fault`; `prop_seek_eof_boundary` |
| `HAL-FS-004` | `eof(handle)` | valid handle | returns 1 when `position >= len` else 0 | invalid handle -> `AdapterFault` | implemented-verified | `file_open_seek_eof_lof_close_roundtrip` |
| `HAL-FS-005` | `lof(handle)` | valid handle | returns logical length token | invalid handle -> `AdapterFault` | implemented-verified | `file_open_seek_eof_lof_close_roundtrip` |
| `HAL-FS-006` | `free_file(range_selector)` | capability supported | returns first free handle in `[1..255]` or `[256..511]` | no free handle -> `AdapterFault` | implemented-verified | `free_file_respects_low_and_high_ranges`; `free_file_low_range_tracks_allocated_handles`; `prop_free_file_low_range_tracks_open_count` |
| `HAL-FS-007` | v1 file model is deterministic in-memory handle semantics, not OS file binding semantics. | none | behavior deterministic and testable | n/a | implemented-partial | specified in implementation-defined registry |

### 6.4 `ProcessEnvHal`

| Clause ID | Operation | Preconditions | Postconditions | Failure obligations | Status | Verification |
|---|---|---|---|---|---|---|
| `HAL-PROC-001` | `shell(command, style)` | capability supported; process spawn allowed | deterministic token return | unsupported/policy-denied | implemented-verified | conformance `process.shell`; host compile-time policy test; `process_com_dynlink_policy_denials_are_enforced` |
| `HAL-PROC-002` | `environ(key)` | capability supported | deterministic token mapping | unsupported -> `CapabilityUnavailable` | implemented-verified | `process_env_deterministic_projection_contract` |
| `HAL-PROC-003` | `dir(path, attrs)` | capability supported | deterministic token mapping | unsupported -> `CapabilityUnavailable` | implemented-verified | `process_env_deterministic_projection_contract` |

### 6.5 `ComHal`

| Clause ID | Operation | Preconditions | Postconditions | Failure obligations | Status | Verification |
|---|---|---|---|---|---|---|
| `HAL-COM-001` | `create_object(prog_id)` | capability supported; COM activation allowed | deterministic token result in v1 | unsupported/policy-denied | implemented-verified | conformance `com.create_object`; host mode tests; `process_com_dynlink_policy_denials_are_enforced` |
| `HAL-COM-002` | `dispatch_invoke(object, member, arg)` | capability supported; COM activation allowed | deterministic token result in v1 | unsupported/policy-denied | implemented-verified | `dispatch_invoke_deterministic_projection_contract`; policy test via `process_com_dynlink_policy_denials_are_enforced` |
| `HAL-COM-003` | Non-Windows profiles must report COM unsupported in v1 descriptor baseline. | profile != windows | deterministic unsupported contract | n/a | implemented-verified | `windows_declares_com_supported_only_on_windows` |

### 6.6 `TimeLocaleHal`

| Clause ID | Operation | Preconditions | Postconditions | Failure obligations | Status | Verification |
|---|---|---|---|---|---|---|
| `HAL-TIME-001` | `date_serial_now`, `time_serial_now`, `timer_ticks` | capability supported | deterministic value tokens in v1 | unsupported -> `CapabilityUnavailable` | implemented-verified | conformance `time.timer_ticks`; `time_locale_contract_values_are_stable` |

### 6.7 `DynamicLinkHal`

| Clause ID | Operation | Preconditions | Postconditions | Failure obligations | Status | Verification |
|---|---|---|---|---|---|---|
| `HAL-DYN-001` | `invoke_symbol(symbol, arg)` | capability supported; dynamic link allowed by policy | deterministic token result | unsupported/policy-denied | implemented-verified | conformance `dynlink.invoke_symbol`; `process_com_dynlink_policy_denials_are_enforced` |

### 6.8 `DiagnosticsHal`

| Clause ID | Operation | Preconditions | Postconditions | Failure obligations | Status | Verification |
|---|---|---|---|---|---|---|
| `HAL-DIAG-001` | `emit(code, payload)` | capability supported | deterministic token result | unsupported -> `CapabilityUnavailable` | implemented-verified | conformance `diag.emit`; `diagnostics_emit_contract_is_deterministic` |

## 7. Null Profile Clauses

| Clause ID | Clause | Status | Verification |
|---|---|---|---|
| `HAL-NULL-001` | Null profile is a deterministic unsupported floor for non-guaranteed capabilities. | implemented-verified | conformance across profiles |
| `HAL-NULL-002` | Null profile may still support explicitly declared deterministic capabilities (`TimeLocale`, `DiagnosticsTelemetry` in v1). | implemented-verified | `null_profile_support_set_is_explicit`; descriptor + conformance |

## 8. Verification Coverage Summary

| Coverage band | Meaning |
|---|---|
| `verified-core` | Clause has direct executable coverage in HAL or host tests. |
| `specified-needs-tests` | Clause documented, implementation exists, but dedicated clause-level test is not yet present. |
| `specified-pending` | Clause defined as target behavior for future implementation. |

Phase-3 interim aggregate:
- verified-core: descriptor/policy/error floor + clause-mapped probes + per-method deterministic checks + property checks for selected filesystem invariants + runtime error-routing guarantees.
- specified-needs-tests: advanced behavioral parity clauses (host-native semantics, queue fairness, ABI-stability guarantees).
- specified-pending: richer host semantics not yet formalized for parity claims.
