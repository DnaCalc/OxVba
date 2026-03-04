# COM Client/Server Conformance V1

Status: `design-draft`  
Date: 2026-03-04  
Companion scope: `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`

## 1. Goal

Define executable conformance lanes for Windows COM client/server support with deterministic evidence capture and strict contract traceability.

## 2. Conformance Lanes

### Lane L0: Contract floor (platform-agnostic checks)

Purpose:

- Ensure unsupported profiles fail deterministically for COM operations.
- Validate error-code and payload invariants independent of native COM calls.

Primary checks:

- `HAL-COM-*` clause guards.
- VM/host error routing shape invariants.

### Lane L1: Windows isolated COM client lane (registration-free)

Purpose:

- Validate native COM activation/invoke in controlled local test setup without registry dependency.

Primary checks:

- apartment init lifecycle,
- `CreateObject`/activation wrapper behavior,
- `GetIDsOfNames`/`Invoke` scalar packing path,
- HRESULT to OxVba diagnostic mapping.

### Lane L2: Windows registered COM client lane

Purpose:

- Validate realistic ProgID/CLSID activation path.

Primary checks:

- `CreateObject` with registered test classes,
- deterministic behavior for class-not-registered and method-not-found paths,
- argument/result conversion subset parity.

### Lane L3: Windows COM server scaffold lane

Purpose:

- Validate Rust-based automation server skeleton exposed by OxVba artifacts.

Primary checks:

- class factory and instance activation,
- `IDispatch` member resolution,
- method call return/error routing,
- object lifetime cleanup at harness shutdown.

### Lane L4: End-to-end OxVba lane

Purpose:

- Validate VBA source invoking COM client paths and external host invoking OxVba COM server paths.

Primary checks:

- VBA -> VM -> HAL -> COM server roundtrip,
- COM host -> OxVba COM server -> runtime route roundtrip,
- deterministic diagnostics under policy-denied and unsupported modes.

## 3. Harness Architecture (Planned)

### Rust test components

Planned module families:

- `crates/oxvba-com/tests/windows_client_*`
- `crates/oxvba-com/tests/windows_server_*`
- `crates/oxvba-host/tests/com_integration_*`

### Conformance fixtures

Planned fixture roots:

- `conformance/com/client/`
- `conformance/com/server/`
- `conformance/com/roundtrip/`

### Execution scripts

Planned script surfaces:

- `scripts/run-com-conformance.ps1` (root orchestrator)
- `scripts/run-com-registrationless.ps1` (isolated lane)
- `scripts/run-com-registered.ps1` (registered lane)

## 4. Artifact Model

Planned evidence paths:

- `docs/evidence/conformance/com/<timestamp>/results.csv`
- `docs/evidence/conformance/com/<timestamp>/summary.md`
- `docs/evidence/conformance/com/COM_CONFORMANCE_LATEST.csv`
- `docs/evidence/conformance/com/COM_CONFORMANCE_LATEST.md`

Each row should include:

- clause ID(s),
- lane ID,
- test ID,
- profile/runtime class,
- pass/fail/skip/deferred status,
- error code (if failure),
- repro command.

## 5. Formal and Property Lanes

Kani/property scope (deferred-gate eligible):

1. argument pack mapping invariants for dispatch invoke.
2. handle-lifetime state machine invariants.
3. HRESULT classification totality (no unmapped terminal paths).
4. deterministic error translation pre/post conditions.

Policy:

- Formal failures are non-blocking unless they indicate memory-safety unsoundness or invariant contradiction with implemented behavior.

## 6. Deferred-Oracle Integration

Required deferred-oracle topics:

1. optional/named argument parity across Office-hosted automation servers.
2. EXCEPINFO and ArgErr parity in edge-case invoke failures.
3. class-module COM exposure parity against VBA host behavior.

Tracked via:

- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`
- linked divergence records in `docs/evidence/divergences/`.

