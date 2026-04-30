# Web Host Shell Baseline Evidence

Date: 2026-04-03
Scope: desktop-first web shell baseline over the typed OxVba web host bridge

## Delivered Baseline

Current crates:
- `crates/oxvba-web-host`
- `crates/oxvba-web-shell`

Current bounded behavior:
- typed serializable bridge contract for workspace/session, run/reset, immediate, and debug pause projection
- desktop-first shell process with embedded frontend asset inventory
- first-pass OxIde frankentui frontend screens for workspace, editor,
  diagnostics, immediate, and debugger views
- shell session command handling for:
  - workspace load/reload
  - document listing
  - document text update / close
  - diagnostics projection
  - run project
  - reset runtime
- bounded Immediate Window evaluation against a live runtime session
- headless screen audit that captures each designed screen and probes the
  generated images for non-blank render output

## Validation Evidence

Commands executed in this delivery lane:

```powershell
cargo check -p oxvba-web-host
cargo test -p oxvba-web-host -- --nocapture
cargo check -p oxvba-web-shell
cargo test -p oxvba-web-shell -- --nocapture
cargo run -p oxvba-web-shell -- --dump-shell-manifest
./scripts/check-governance.ps1
./scripts/audit-web-shell-screens.ps1
```

Observed proof points:
- bridge DTOs serialize and deserialize cleanly
- workspace-loaded event projection works
- immediate-result projection works
- debug pause projection works
- shell asset manifest is embedded and inspectable
- shell session loads a real `.basproj` workspace and emits diagnostics events
- shell session runs and resets a real project
- shell session evaluates an immediate command against the live runtime session
- screen audit passed for all five first-pass frankentui screens, with captures
  under `docs/evidence/web-shell/screen-audit-latest/`

Current screen audit:
- `docs/evidence/web-shell/screen-audit-latest/screen-audit.md`
- `docs/evidence/web-shell/screen-audit-latest/screen-audit.json`
- `docs/evidence/web-shell/screen-audit-latest/workspace.png`
- `docs/evidence/web-shell/screen-audit-latest/editor.png`
- `docs/evidence/web-shell/screen-audit-latest/diagnostics.png`
- `docs/evidence/web-shell/screen-audit-latest/immediate.png`
- `docs/evidence/web-shell/screen-audit-latest/debugger.png`

## Honest Boundary

This baseline does not yet claim:
- browser-native wasm packaging
- JS/WASM callback ABI realization
- a production desktop container technology choice
- full IDE parity
- live browser-to-Rust command transport from the static frontend controls

See also:
- `docs/spec/WEB_HOST_BRIDGE_CONTRACT_V1.md`
- `docs/spec/BROWSER_NATIVE_WASM_HANDOFF_V1.md`

It is the substrate that proves the desktop-first host-shell direction is real and that the later browser-native wasm lane now has a clean handoff boundary instead of only planning text.
