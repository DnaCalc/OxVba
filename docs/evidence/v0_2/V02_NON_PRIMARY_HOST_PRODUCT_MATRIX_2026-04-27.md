# V0.2 Non-Primary Host Product-Truth Matrix

Date: 2026-04-27

Bead: `bd-bqm8.9.3`

## Rule

Windows remains the primary V0.2 host for native COM, Office automation, and
Windows-specific VBA compatibility claims. Linux, macOS, and wasm/WASI are
validated as non-primary hosts for portable build, governance, deterministic
runtime, and HAL conformance surfaces only.

## Matrix

| ID | Host or Surface | V0.2 Claim | Active Validation | Boundary |
| --- | --- | --- | --- | --- |
| NPH-V02-001 | Windows | Primary host lane for Windows COM, Office-oriented behavior, and the standard ready job. | `windows-ready` runs `./scripts/meta-check.ps1 -Fast -NoArtifacts`; native HAL tests cover Windows COM paths when available. | Windows-specific parity remains governed by the active VBA 7.1/Office ladder and its oracle/follow-up rows. |
| NPH-V02-002 | Linux | Non-primary portable build/governance lane. | `linux-ready` runs `./scripts/meta-check.ps1 -Fast -NoArtifacts` on `ubuntu-latest`. | No native Office automation or COM activation parity is claimed. |
| NPH-V02-003 | macOS | Non-primary portable build/governance lane. | `macos-ready` runs `./scripts/meta-check.ps1 -Fast -NoArtifacts` on `macos-latest`. | No native Office automation or COM activation parity is claimed. |
| NPH-V02-004 | wasm32/WASI | HAL conformance executable builds and runs under wasm/WASI. | `wasm-hal-ready` runs `./scripts/run-hal-conformance-wasm32.ps1 -SkipTests -OutputDir temp/no-artifacts/hal_wasm32_ci`. Local evidence: `HAL_CONFORMANCE_1777280393.md` and `.jsonl`. | Full wasm-target unit-test parity is not claimed because current host/unit tests include Windows-native COM helpers. |
| NPH-V02-005 | Non-Windows COM and typelib resolution | Deterministic unsupported behavior, not native automation. | Non-Windows COM bridge paths return explicit unsupported errors for dynamic library loading, symbol lookup, and live ProgID/typelib resolution. | Native COM, Office object models, connection points, and real typelib loading remain Windows-only. |
| NPH-V02-006 | Browser-style wasm shell | Runtime class is modeled and conformance-visible. | HAL descriptor/conformance paths cover `browser-sandbox` as a wasm runtime class. | Browser packaging and full web IDE parity are not V0.2 claims. |

## Product Language

Allowed V0.2 language:

- "Linux and macOS are actively validated as portable non-primary build and
  governance hosts."
- "wasm32/WASI actively builds and runs the HAL conformance executable."
- "Non-Windows native COM and Office automation are unsupported by design in
  V0.2 and fail through deterministic unsupported paths."

Disallowed V0.2 language:

- "Linux/macOS have Office automation parity."
- "wasm has full crate unit-test parity."
- "Browser wasm is a complete IDE/runtime packaging target."
- "Non-Windows hosts implement live COM activation or typelib loading."

## Evidence Links

- Active validation rollout:
  [V02_NON_PRIMARY_HOST_CI_VALIDATION_2026-04-27.md](/C:/Work/DnaCalc/OxVba/docs/evidence/v0_2/V02_NON_PRIMARY_HOST_CI_VALIDATION_2026-04-27.md)
- HAL architecture summary:
  [ARCHITECTURE.md](/C:/Work/DnaCalc/OxVba/docs/ARCHITECTURE.md)
