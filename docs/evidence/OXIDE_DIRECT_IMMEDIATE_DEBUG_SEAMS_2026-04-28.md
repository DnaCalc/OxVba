# OxIde Direct Immediate/Debug Seams

Date: 2026-04-28
Beads: `bd-iyx4.1.2`, `bd-iyx4.1.3`
Workset: `docs/worksets/WORKSET_2026-04-28_OXIDE_XLL_ARRAY_APPLICATION_EXECUTION.md`

## Claim

OxVba now has a focused OxIde-class direct-host proof for the Immediate Window
and debugger seams. The proof consumes `oxvba-host` APIs directly and does not
route through the CLI, LSP, VS Code, web-shell projection, or placeholder
surfaces.

## Added Test

```text
crates/oxvba-host/tests/oxide_direct_host_consumption.rs
```

Coverage:

- `oxide_direct_immediate_window_consumes_live_session_without_cli_lsp_or_placeholder`
  creates an `Engine`, prepares a live runtime session, wraps it in
  `ImmediateSession`, evaluates through `evaluate_variant`, proves retained
  `Variant` string output, proves live session state with a second immediate
  call, and checks retained snapshot content.
- `oxide_direct_debug_seam_consumes_variant_pause_and_eval_without_cli_lsp_or_placeholder`
  creates a `DebugSession`, starts and steps with the retained
  `*_variants` debugger APIs, checks frame/stop state, and evaluates a paused
  frame identifier through `evaluate_variant`.

## Validation

Command:

```powershell
cargo test -p oxvba-host --test oxide_direct_host_consumption --quiet
```

Result:

```text
running 2 tests
..
test result: ok. 2 passed; 0 failed
```

## Boundary

This is OxVba-side proof for an OxIde-class direct host. It does not claim the
OxIde UI has wired panes, commands, focus handling, transcript rendering, or
project reload policy. Those remain OxIde-side integration work, now backed by
direct OxVba seams instead of CLI/LSP placeholder assumptions.
