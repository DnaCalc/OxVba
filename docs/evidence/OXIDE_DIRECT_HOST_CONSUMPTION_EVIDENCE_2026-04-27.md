# OxIde Direct Host Consumption Evidence

Date: 2026-04-27
Bead: `bd-oxi1.6.2`
External checkout inspected: `C:\Work\DnaCalc\OxIde`

## Evidence Captured

The local OxIde checkout contains direct OxVba consumption in:

- `src/shell/oxvba.rs`
  - imports `oxvba_languageservice::HostWorkspaceSession`
  - loads workspace semantics through `HostWorkspaceSession::load_workspace_path`
  - uses direct diagnostics, document symbols, hover, and go-to-definition APIs
  - routes build/run-style execution through OxVba runtime/web-host contracts
- `src/shell/project_actions.rs`
  - imports `oxvba_project` host-helper surfaces
  - uses project edit planning/application and COM reference helper flows
- `ARCHITECTURE.md`
  - states OxIde's intended boundary: direct embedding of OxVba project/session
    semantics, not LSP-shaped or CLI-shaped language intelligence

## Validation Commands

Run from `C:\Work\DnaCalc\OxIde`:

```powershell
cargo test real_oxvba --quiet
cargo test project_actions --quiet
```

Results:

- `cargo test real_oxvba --quiet`: pass, 5/5 filtered tests
- `cargo test project_actions --quiet`: pass, 8/8 filtered tests

## Remaining Boundary

This evidence proves real OxIde consumption of direct workspace/language-service
and project-helper surfaces, plus runtime build/run-facing integration tests.

It does not yet prove direct OxIde consumption of the full direct Immediate
Window/debug seams named in `bd-oxi1.6.2`. The current OxIde docs still frame
Immediate/debug as planned surfaces or future OxVba-contract-dependent work.
That remaining portion should stay blocked until OxIde has code and tests that
exercise those direct APIs rather than CLI, LSP, or placeholder UI flows.
