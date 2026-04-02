# Workset: Web/Wasm Host-Shell Realization Execution

Date: 2026-04-02
Owner: Codex
Status: in-progress

## Purpose

Advance the web/wasm theme from review/planning into a concrete host-shell execution lane with explicit bridge and showcase boundaries.

## Why This Exists

The earlier review work concluded that the honest near-term path is:
- desktop-first shell,
- Rust backend,
- web UI frontend,
- embedded OxVba engine,
- explicit host bridge.

That recommendation now needs a current execution owner and bead root.

## Governing Policy

1. The first product lane is desktop-first host shell, not browser-native wasm.
2. The host bridge must stay explicit and typed.
3. Diagnostics, run/reset control, and immediate/debug-style commands are the first useful shell slice.
4. Browser-native wasm remains a later follow-on.

## Required Outcomes

1. A desktop-first shell can load a supported OxVba workspace.
2. The host bridge can run/reset and surface diagnostics/output.
3. At least one immediate/debug-style command path exists.
4. Validation rows and evidence exist for the shell baseline.

## Execution Slices

1. define the typed host bridge
2. build the shell baseline
3. wire project load/run/reset
4. route diagnostics/output and minimal commands
5. publish validation evidence
6. only then plan browser-native wasm follow-ons

Current execution state:
- this workset is now the active execution owner for the remaining ready web/wasm lane
- the typed desktop-first host bridge contract is now landed as the first substrate
- shell packaging and frontend wiring remain the next real delivery lane over that explicit bridge

## Relationship To Prior Work

This workset continues:
- `WORKSET_2026-03-30_WEB_WASM_DESKTOP_FIRST_HOST_SHELL_AND_BRIDGE_FOUNDATION.md`

The earlier review work remains the planning provenance; this is the current execution owner.

## Non-Goals

- browser-native wasm product delivery
- full IDE parity
- rich form designer or host object model

## Exit Condition

This workset is complete only when:
- the desktop-first host shell is real and evidenced,
- the bridge is explicit and validated,
- and the next browser-native wasm lane is unblocked honestly rather than aspirationally.
