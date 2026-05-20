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
cargo check -p oxvba-languageservice --target wasm32-unknown-unknown --no-default-features
cargo check -p oxvba-web-host --target wasm32-unknown-unknown --no-default-features
```

Installed target evidence:

- `wasm32-unknown-unknown` is installed.

## Result

Initial status: blocked for the default browser crate graph.

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

Delivery status after `bd-5wjn`: browser-safe subset is feature-gated and
checkable.

The following checks pass:

```powershell
cargo check -p oxvba-languageservice --target wasm32-unknown-unknown --no-default-features
cargo check -p oxvba-web-host --target wasm32-unknown-unknown --no-default-features
```

The split is explicit:

- `oxvba-languageservice` now gates `host_session` behind a default
  `host-session` feature; no-default builds keep the core language-service
  graph without `oxvba-host`/`oxvba-project`.
- `oxvba-web-host` now gates native runtime/debug/Immediate projection helpers
  behind a default `native-host` feature; no-default builds keep serializable
  browser DTOs and language-service projection types without `oxvba-host`.
- `oxvba-com` gates the native FFI bridge and `libc` dependency out for
  `wasm32`, avoiding the unavailable `dlopen`/`dlsym` path in browser checks.

## Gap Classification

| Surface | Current state | Browser-safe gap | Delivery owner |
| --- | --- | --- | --- |
| `oxvba-languageservice` | Default feature includes host-session native/project integration. No-default feature graph passes `wasm32-unknown-unknown`. | Browser callers must use no-default features until host-session is split into browser-safe and native subprofiles. | delivered subset |
| `oxvba-web-host` | Default feature includes native host runtime/debug/Immediate projection helpers. No-default feature graph passes `wasm32-unknown-unknown`. | Typed unavailable packets for runtime/debug/COM/native command families still need a browser command-response contract. | delivered subset / taxonomy follow-up |
| `oxvba-web-shell` | Depends on `oxvba-host`, `oxvba-project`, and native runtime session wiring. | Needs a separate shell/browser feature split before a WASM shell build can be claimed. | future |

## Non-Claims

This check does not claim:

- current browser/WASM build support for the full OxIde shell;
- runtime/debug/COM availability in browser;
- that the failure is a `region` bug to patch locally;
- that passing HAL wasm evidence proves the direct web-host crate graph.

The default native feature graph still intentionally pulls native host/JIT
dependencies. Browser checks must use the no-default browser-safe subset until
the shell/runtime command families are split further.

## Follow-Up

Delivered in `bd-5wjn`:

- feature-gated language-service and web-host browser-safe checks;
- `wasm32` gating for the COM native FFI bridge.

Remaining future work:

- browser-shell feature split;
- typed browser unavailable packets for runtime/debug/COM/native command
  families.
