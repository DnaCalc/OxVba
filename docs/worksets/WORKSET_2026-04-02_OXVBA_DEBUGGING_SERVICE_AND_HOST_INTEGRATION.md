# Workset: OxVba Debugging Service And Host Integration

Date: 2026-04-02
Owner: Codex
Status: in-progress

## Purpose

Define and deliver the first real OxVba debugging substrate for host IDEs, with OxIde as the direct showcase consumer and VS Code as a later DAP consumer.

## Why This Exists

We now have:
- direct host sessions,
- project and reference helper APIs,
- a thin LSP shell,
- and a clear split between semantic truth and host UI.

What we do not yet have is a debugger model.
The project needs one coherent answer for:
- breakpoints,
- stepping,
- locals and watches,
- call stacks,
- procedure entry/exit,
- and expression evaluation against OxVba state.

## Debugging Policy

1. We are debugging OxVba code, not native Rust machine code.
2. The primary debugger must be semantic and source-oriented:
   - statement-level breakpoints,
   - procedure-level stack frames,
   - OxVba locals/arguments/watch values.
3. The first debugger lane should be cross-platform in principle.
4. Windows-specific debugging helpers are acceptable for COM/Office-hosted scenarios, but they are add-ons, not the main model.
5. Do not make LLDB/GDB/CDB/native-stack stepping a product requirement.
6. Prefer a VM/interpreter-backed debug mode first; JIT debugging can deopt, instrument, or temporarily fall back to the VM as needed.

## Appropriate Options

### Primary recommended option

Build a direct OxVba debugger core inside the runtime/host stack:
- source position tables and statement identities,
- breakpoint registration and hit reporting,
- step into / over / out,
- call stack and locals inspection,
- watch/eval in the current OxVba context.

Why:
- portable,
- semantics-accurate,
- no native symbol/debugger dependency,
- reusable across OxIde and VS Code.

### Secondary projection options

- OxIde:
  - direct Rust host binding to the debugger core
- VS Code:
  - DAP adapter over the same debugger core

### Windows-only add-on options

Useful later for Office/COM scenarios:
- Office-hosted attach/run-control harnesses
- COM activation/session diagnostics
- host-object and event tracing

These are useful, but they should layer on top of the semantic debugger rather than replace it.

## Required Outcomes

1. A typed OxVba-side debug session API exists.
2. Breakpoints are source/statement based, not native-PC based.
3. Hosts can inspect:
   - stack
   - locals
   - watches
   - current statement/source span
4. The debug substrate is usable directly from OxIde.
5. The same substrate can later be projected into DAP for VS Code.

## Execution Slices

1. define the semantic debugger contract
2. land source-position and statement identity support needed for breakpoints/stepping
3. implement VM-backed debug execution mode
4. expose typed host-facing debug session APIs
5. add OxIde-facing debug harness/evidence
6. later add DAP projection for VS Code
7. later add Windows-specific COM/Office debug helpers where justified

Current execution state:
- workset and policy are published
- debugger contract/spec is now published in `docs/spec/OXVBA_DEBUGGER_CONTRACT_V1.md`
- the next delivery slice is the source/statement identity substrate for breakpoints and stepping

## Non-Goals

- native instruction-level stepping
- native stack inspection as the primary UX
- first-pass JIT-native debugger parity
- Edit and Continue
- time-travel debugging

## Relationship To Other Worksets

- OxIde direct host parent:
  - `WORKSET_2026-04-01_OXIDE_HOST_SURFACE_AND_VSCODE_ALTERNATE_EDITOR_EXECUTION.md`
- VS Code extension lane:
  - `WORKSET_2026-04-02_VSCODE_EXTENSION_AND_LSP_FEATURE_LADDER_EXECUTION.md`

## Exit Condition

This workset is complete only when:
- the OxVba debugger core exists and is host-consumable,
- OxIde can debug OxVba code without native debuggers,
- and the path to a VS Code DAP adapter is explicit and technically credible.
