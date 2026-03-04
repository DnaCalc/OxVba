# PROFILE_LADDER_2026-03-04_MACH1000_V287_V306_COM_FORMAL_SCAFFOLD

## Range

- Ladder span: `v287..v306`
- Focus: COM formal baseline and first Windows native client/server scaffold.

## Objective

Establish a rigorous COM foundation for OxVba:

1. formal scope and clause contracts,
2. executable conformance scaffolding,
3. initial Windows native COM client/server smoke path.

## Authoritative Inputs

- `CHARTER.md`
- `OPERATIONS.md`
- `MACH1000_PLAN.md`
- `docs/spec/COM_CLIENT_SERVER_SCOPE_V1.md`
- `docs/spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md`
- `docs/spec/HAL_COM_BRIDGE_SCOPE_V1.md`
- `docs/spec/CLASS_MODULE_COM_ALIGNMENT_PLAN_V1.md`

## Profile Steps

| Step | Focus | Deliverables |
|---|---|---|
| `v287` | Source/anchor baseline | COM source crosswalk baseline and implementation-defined topic capture refresh. |
| `v288` | COM clause catalog skeleton | `COM-*` clause IDs with pre/post/failure obligations (`md` + `csv`). |
| `v289` | Error taxonomy extension | deterministic COM error families and mapping policy in diagnostic taxonomy. |
| `v290` | Apartment policy contract | host policy/config contract for COM apartment lifecycle and deterministic rejection behavior. |
| `v291` | Lifetime model formalization | object-token/refcount lifecycle invariants and shutdown guarantees. |
| `v292` | Conformance harness contract | lane definitions, artifact schema, and gating policy finalized for COM series. |
| `v293` | Test component scaffold | Rust COM test component skeleton (client-target and server-target) created. |
| `v294` | Windows harness scripts | script scaffolding for registration-free and registered COM test lanes. |
| `v295` | Client fixture pack v0 | initial conformance fixtures for `CreateObject` + scalar dispatch subset. |
| `v296` | Server fixture pack v0 | initial fixtures for OxVba COM server scaffold activation/invocation subset. |
| `v297` | Windows adapter lifecycle | COM init/uninit lifecycle scaffold in Windows adapter path. |
| `v298` | Native activation path | first native `CreateObject` path for test components with deterministic errors. |
| `v299` | Native invoke path (scalar) | first `GetIDsOfNames` + `Invoke` scalar call path. |
| `v300` | Variant scalar marshaling | scalar `VARIANT`/`BSTR` in-out mapping subset with stable contracts. |
| `v301` | HRESULT mapping table | deterministic HRESULT/EXCEPINFO/ArgErr translation baseline. |
| `v302` | COM server class factory scaffold | Rust server class registration/factory scaffolding for tests. |
| `v303` | COM server dispatch scaffold | minimal `IDispatch` method surface for deterministic smoke tests. |
| `v304` | Host policy integration | compile/runtime policy controls for COM server/client lanes. |
| `v305` | Integrated COM smoke gate | execute L0-L4 smoke subset; publish initial COM conformance artifacts. |
| `v306` | Terminal closure gate | final docs sync, evidence rollup, and handoff to `v307..v336`. |

## Gate Policy

- Formal/Kani checks are non-blocking by default (unless unsoundness risk is detected).
- Windows-only native COM assertions must be explicit and profile-gated.
- Non-Windows COM behavior must remain deterministic unsupported.

## Exit Criteria (`v306`)

1. COM scope and conformance docs are clause-linked and executable-lane ready.
2. Windows native COM client path executes first smoke subset (`CreateObject` + scalar `Invoke`).
3. OxVba COM server scaffold can be activated and invoked in test harness.
4. Artifact outputs exist under `docs/evidence/conformance/com/` for the closure run.

