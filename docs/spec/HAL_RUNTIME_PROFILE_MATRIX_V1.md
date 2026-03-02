# HAL Runtime Profile Matrix V1

Status: `working-draft`  
Step: `v187`  
Date: 2026-03-02

## Objective

Define a single runtime taxonomy that separates:
- platform/runtime class identity,
- capability declaration,
- policy behavior.

This doc is the baseline for host runner configuration and conformance coverage.

## Terms

- Profile: semantic host class selected by runtime (`windows-gui`, `windows-headless`, `linux-stdio`, `wasm-wasi-local`, `wasm-browser-sandbox`, `null-floor`).
- Policy: execution permissions and determinism controls (`strict-ci`, `deterministic-runtime`, `deterministic-compile-time`, `interactive-dev`).
- Capability: HAL domain surfaced by descriptor.

## Profile Set (V1)

1. `windows-gui`
2. `windows-headless`
3. `linux-stdio`
4. `wasm-wasi-local`
5. `wasm-browser-sandbox`
6. `null-floor`

## Capability Matrix (V1 intent)

| Profile | UI | EventPump | FileSystemIo | ProcessEnv | COM | TimeLocale | DynamicLinking | Diagnostics |
|---|---|---|---|---|---|---|---|---|
| `windows-gui` | supported | supported | supported | supported | supported | supported | supported | supported |
| `windows-headless` | supported (virtualized/headless) | supported | supported | supported | supported | supported | supported | supported |
| `linux-stdio` | supported (stdio mode) | supported | supported | supported | unsupported | supported | supported | supported |
| `wasm-wasi-local` | supported (virtualized) | supported | unsupported | unsupported | unsupported | supported | unsupported | supported |
| `wasm-browser-sandbox` | unsupported | supported | unsupported | unsupported | unsupported | supported | unsupported | supported |
| `null-floor` | unsupported | unsupported | unsupported | unsupported | unsupported | supported | unsupported | supported |

## Policy Interaction

Policy is orthogonal to profile:
- profile answers "can this domain exist here?",
- policy answers "is it allowed this run, and deterministic or host-backed?".

Examples:
- `windows-gui + deterministic-runtime`: UI calls remain virtualized deterministic.
- `windows-gui + interactive-dev`: UI may use native host behavior.
- `linux-stdio + strict-ci`: prompts denied or deterministic fallback according to policy.

## Descriptor Requirements

Descriptor for each run must include:
- `profile` (runtime profile),
- `runtime_class` (host-native/wasi/browser-sandbox/null-floor),
- `contract_version`,
- `adapter_version`,
- per-capability support + maturity + spec anchor.

## Conformance Requirements

Every profile must run all lanes:
1. runtime
2. compile-time
3. interactive-dev

WASM profiles must include runtime-class specific rows.

## Open Items

- mapping existing `HalProfileId` enum values to the above profile set in host runner UX.
- whether `windows-gui` and `windows-headless` should be separate enum variants or runtime-class overlays.
- final naming lock belongs to host runner design step (`v195`).
