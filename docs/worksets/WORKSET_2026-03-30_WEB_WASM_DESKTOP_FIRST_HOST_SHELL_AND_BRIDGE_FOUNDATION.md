# Workset: Web/Wasm Desktop-First Host-Shell And Bridge Foundation

Date: 2026-03-30  
Status: proposed  
Scope: execute the first real host-shell and host-bridge baseline for the web/wasm theme, using a desktop-first shell with Rust backend and web UI frontend while preserving a clean future browser-native wasm path.

## 1. Purpose

This workset is the recommended next execution workset produced by the review umbrella:
[WORKSET_2026-03-30_WEB_WASM_HOSTED_REALIZATION_REVIEW_AND_SHOWCASE_PLANNING.md](/C:/Work/DnaCalc/OxVba/docs/worksets/WORKSET_2026-03-30_WEB_WASM_HOSTED_REALIZATION_REVIEW_AND_SHOWCASE_PLANNING.md)

It exists to convert the reviewed recommendation into an execution boundary that can be implemented and validated honestly.

## 2. Execution Target

The target realization is:
1. desktop-first host shell
2. Rust backend
3. web UI frontend
4. embedded OxVba engine
5. explicit host bridge for diagnostics, commands, and host-event ingress

This workset is not the browser-native `oxvba.wasm` product lane.

## 3. Required Outcomes

This workset should only be considered complete when all of the following are true:
1. the host-bridge contract is explicit and validated
2. the shell can open/load a supported OxVba project
3. the shell can run and reset the engine under host control
4. diagnostics/output route into the shell
5. at least one minimal immediate/debug-style command path is demonstrated
6. validation rows and evidence exist for the shell/bridge baseline

## 4. Major Work Areas

1. host-bridge contract definition
2. shell baseline implementation
3. project load/run/reset execution flow
4. diagnostics and event ingress plumbing
5. validation matrix extension and evidence capture

## 5. Explicit Non-Goals

This workset should not claim:
1. browser-native wasm artifact delivery
2. JS/WebAssembly callback ABI completion
3. full IDE parity
4. form designer or rich UI object model
5. broad web-hosted product closure

## 6. Follow-On Relationship

If this workset succeeds, the next later workset may target:
1. browser-native wasm packaging,
2. explicit JS/WebAssembly host-bridge realization,
3. browser-hosted demonstration and validation lanes.
