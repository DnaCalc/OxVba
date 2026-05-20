# DNA OxIde After-W370 Browser/WASM Crate-Graph Check

Date: 2026-05-20
Bead: `bd-94av.4.1`
Workset: `docs/worksets/WORKSET_2026-05-19_DNAOXIDE_AFTER_W370_DIRECT_HOST_ROUNDOUT.md`

## Scope

This check covers the intended OxIde browser subset: workspace/document load,
language-service/check surfaces, and web-host DTO projection. Runtime, debug,
COM, JIT, filesystem-native, and process-native services are not claimed as
browser-available unless they pass behind explicit feature gates.

## Commands Run

```powershell
rustup target list --installed
cargo check -p oxvba-web-host --target wasm32-unknown-unknown
cargo check -p oxvba-languageservice --target wasm32-unknown-unknown
cargo check -p oxvba-web-shell --target wasm32-unknown-unknown
cargo tree -p oxvba-web-host --target wasm32-unknown-unknown -i region
cargo tree -p oxvba-languageservice --target wasm32-unknown-unknown -i oxvba-host
```

Installed target evidence:

- `wasm32-unknown-unknown` is installed.

## Result

Status: blocked for the current browser crate graph.

`cargo check -p oxvba-web-host --target wasm32-unknown-unknown`,
`cargo check -p oxvba-languageservice --target wasm32-unknown-unknown`, and
`cargo check -p oxvba-web-shell --target wasm32-unknown-unknown` all fail
before OxVBA browser code can be checked because the target graph reaches the
native JIT dependency stack:

```text
region v3.0.2
└── cranelift-jit v0.129.1
    └── oxvba-jit
        └── oxvba-host
            ├── oxvba-languageservice
            ├── oxvba-project
            └── oxvba-web-host
```

The concrete failure is in `region v3.0.2` for `wasm32-unknown-unknown`:
missing OS allocation/protection/query functions such as `os::alloc`,
`os::free`, `os::protect`, `os::page_size`, and `os::QueryIter`.

## Gap Classification

| Surface | Current state | Browser-safe gap | Delivery owner |
| --- | --- | --- | --- |
| `oxvba-languageservice` | Depends on `oxvba-host` and `oxvba-project`; both reach native host/JIT/COM dependencies. | Needs a browser-safe dependency split or feature gate so language-service/check surfaces can build without native runtime/JIT/COM. | `bd-5wjn` |
| `oxvba-web-host` | Depends on `oxvba-host` and uses runtime/debug/Immediate DTOs. | Needs browser profile gates and typed unavailable packets for runtime/debug/COM/native services while keeping workspace/language DTOs buildable. | `bd-5wjn` |
| `oxvba-web-shell` | Depends on `oxvba-web-host`, `oxvba-host`, `oxvba-project`, and native runtime session wiring. | Needs shell/browser feature split before a WASM shell build can be claimed. | `bd-5wjn` |

## Non-Claims

This check does not claim:

- current browser/WASM build support for OxIde;
- runtime/debug/COM availability in browser;
- that the failure is a `region` bug to patch locally;
- that passing HAL wasm evidence proves the direct web-host crate graph.

The current blocker is OxVBA-owned dependency shape: browser-intended crates
still pull native host/JIT dependencies.

## Follow-Up

Created delivery bead:

- `bd-5wjn` - split browser-safe language-service graph from native host/JIT
  dependencies.

That bead should either make the browser subset pass
`wasm32-unknown-unknown` checks or leave explicit feature-gated unavailable
responses for unsupported native services.
