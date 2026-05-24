# PH-0011 Host Callable Refresh

Date: 2026-05-24
Bead: `bd-hjys.14`
Matrix: `docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv`

## Refreshed status

`PH-0011` now describes neutral host callable reflection/invocation and host-owned UDF admission examples, not core `HostUdf*` APIs.

Updated matrix language:

- `subdomain`: `host_callable`
- feature: neutral host callable reflection/invocation and host-owned UDF admission examples
- truth state: `implemented-subset`
- active evidence: `docs/evidence/host_callable/*` artifacts from `bd-hjys.1` through `bd-hjys.13`

## Suite A-J artifact map

| Suite | Coverage | Evidence |
| --- | --- | --- |
| A | Boundary audit | `docs/evidence/host_callable/BOUNDARY_AUDIT.md` |
| B | Neutral descriptor/API contract | `docs/evidence/host_callable/NEUTRAL_DESCRIPTOR_MODEL.md` |
| C | Compiler reflection descriptors | `docs/evidence/host_callable/BD-HJYS.3_NEUTRAL_REFLECTION_DESCRIPTORS.md` |
| D | Bundle descriptor source of truth | `docs/evidence/host_callable/BUNDLE_DESCRIPTOR_TRUTH.md` |
| E | `VbaHost` load/reflect/prepare facade | `docs/evidence/host_callable/IN_PROCESS_HOST_API.md` |
| F | Callable-ID variant/typed invocation and context observation | `docs/evidence/host_callable/RUNTIME_TYPED_INVOCATION.md` |
| G | Old `HostUdf*` removal | `docs/evidence/host_callable/HOSTUDF_API_REMOVAL.md` |
| H | Wrapper substrate: EXE/native/COM/future-XLL | `WRAPPER_PLAN_ABSTRACTIONS.md`, `WRAPPER_GENERATION_EXE.md`, `WRAPPED_NATIVE_LIBRARY_PROFILE.md`, `COM_XLL_WRAPPER_SUBSTRATE_ALIGNMENT.md` |
| I | Host-owned UDF policy and DnaCalc consumption | `HOST_OWNED_UDF_POLICY_W093.md`, `DNA_CALC_HOST_CONSUMPTION.md` |
| J | Terminal audit | deferred to `bd-hjys.15` |

## Superseded historical evidence

The following evidence remains useful provenance but no longer describes active API truth:

- `docs/evidence/HOST_UDF_W093_METADATA_DESCRIPTOR_2026-05-22.md`
- `docs/evidence/conformance/WRAPPED_COM_SERVER_HOST_UDF_PH0011_2026-05-09.md`
- `docs/evidence/conformance/WRAPPED_COM_SERVER_HOST_UDF_CATALOG_INVOKE_PH0011_2026-05-09.md`

They are superseded by neutral callable descriptor/invocation evidence and the host-owned UDF policy example under `docs/evidence/host_callable/`.

## Explicit deferrals

PH-0011 does **not** claim:

- Excel formula binding/name precedence.
- OxFunc registry mutation/snapshot lifecycle.
- Excel volatile/dependency semantics.
- Array/error result parity.
- XLL execution or Excel registration parity.

Those remain outside this neutral host-callable substrate and require future host/wrapper-specific work.
