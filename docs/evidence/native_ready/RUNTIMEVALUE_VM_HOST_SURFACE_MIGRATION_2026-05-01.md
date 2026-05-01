# RuntimeValue VM/Host Surface Migration Evidence

Date: 2026-05-01
Bead: `bd-pn5i.3` / `cleanout-002 migrate VM and host snapshot invoke surfaces`
Workset: `docs/worksets/WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md`

## Outcome

The VM/host snapshot, invocation, immediate, debugger, embedded, and COM
callback/event surfaces no longer expose `RuntimeValue` as the normal semantic
carrier. Normal execution and observation paths now use retained `Variant`
values. Legacy semantic projections are isolated behind explicit compatibility
modules/traits:

- `oxvba_vm::compat`
  - legacy snapshot helpers;
  - `RuntimeValueCompatVmExt::invoke_procedure_with_values`.
- `oxvba_host::compat`
  - `RuntimeValueCompatProjectSessionExt`;
  - `RuntimeValueCompatEngineExt`;
  - `RuntimeValueCompatImmediateSessionExt` and immediate projection DTOs;
  - `RuntimeValueCompatDebugSessionExt` and debug projection DTOs;
  - `RuntimeValueCompatEmbeddedRunSessionExt` and embedded projection DTOs;
  - `RuntimeValueCompatComEventCallbackExt` and COM callback projection DTOs.

## Surface Changes

### VM

- Retained normal APIs are `execute_and_snapshot_variants*`,
  `Vm::snapshot_variants`, and `Vm::invoke_procedure_with_variants`.
- `Vm::invoke_procedure_with_values` was removed from the inherent API and
  reintroduced only as `oxvba_vm::compat::RuntimeValueCompatVmExt`.
- Remaining `Vm::snapshot`, `snapshot_values`, `snapshot_compat_values`,
  `read_value_slot`, and `write_value_slot` uses are `#[cfg(test)]` compatibility
  helpers for legacy VM assertions.

### Host engine/session/COM event surfaces

- `ProjectRuntimeSession` normal APIs retain `Variant` slots/snapshots.
- Legacy project-session slot/snapshot methods live in
  `RuntimeValueCompatProjectSessionExt`.
- `Engine` normal snapshot/invoke/event APIs retain `Variant`; legacy
  RuntimeValue invocation/snapshot/event methods live in
  `RuntimeValueCompatEngineExt`.
- `ComEventCallbackDispatch` moved out of the normal engine exports into
  `oxvba_host::compat`; retained callback polling/dispatch uses
  `ComEventCallbackVariantDispatch`.

### Immediate/debugger/embedded/web/CLI

- Immediate normal evaluation uses `ImmediateVariantEvaluationResult` and
  `ImmediateVariantEvaluationOutput`; RuntimeValue immediate result DTOs live in
  `oxvba_host::compat`.
- Debugger normal run/pause/evaluation APIs use `HostDebugVariantRunResult`,
  `DebugVariantPauseState`, and `DebugVariantEvaluationResult`; RuntimeValue
  debugger DTOs live in `oxvba_host::compat`.
- Embedded normal invocation APIs use `EmbeddedInvokeProcedureVariantRequest`
  and `EmbeddedInvokeVariantResult`; RuntimeValue embedded DTOs live in
  `oxvba_host::compat`.
- `oxvba-cli`, `oxvba-web-host`, and `oxvba-web-shell` consume retained Variant
  observation shapes and project only display text into UI DTOs.

## Scan Evidence

Commands run after migration:

```text
rg -n "RuntimeValue" crates --glob '*.rs' | wc -l
# 2762

rg -n "RuntimeValue" crates/oxvba-host/src crates/oxvba-vm/src --glob '*.rs' | wc -l
# 1314
```

The remaining count is expected for this bead because later beads own runtime,
HAL, COM, JIT, and residual test/helper families. For the `cleanout-002` scope,
normal public surfaces are reduced to retained Variant APIs; the scan of public
RuntimeValue-like VM/host surfaces now points at explicit compatibility modules
or VM test-only helpers:

```text
crates/oxvba-vm/src/lib.rs:104:    pub trait RuntimeValueCompatVmExt {
crates/oxvba-vm/src/lib.rs:130:    pub fn execute_and_snapshot(bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, String> {
crates/oxvba-vm/src/lib.rs:134:    pub fn execute_and_snapshot_values(bytecode: &Bytecode) -> Result<Vec<RuntimeValue>, String> {
crates/oxvba-host/src/compat.rs:43:pub trait RuntimeValueCompatComEventCallbackExt {
crates/oxvba-host/src/compat.rs:71:pub enum ImmediateEvaluationOutput {
crates/oxvba-host/src/compat.rs:137:pub trait RuntimeValueCompatImmediateSessionExt {
crates/oxvba-host/src/compat.rs:166:pub struct DebugFrameValue {
crates/oxvba-host/src/compat.rs:191:pub enum HostDebugRunResult {
crates/oxvba-host/src/compat.rs:285:pub trait RuntimeValueCompatDebugSessionExt {
crates/oxvba-host/src/compat.rs:350:pub struct EmbeddedInvokeProcedureRequest {
crates/oxvba-host/src/compat.rs:374:pub struct EmbeddedInvokeResult {
crates/oxvba-host/src/compat.rs:441:pub trait RuntimeValueCompatEmbeddedRunSessionExt {
crates/oxvba-host/src/compat.rs:482:pub trait RuntimeValueCompatProjectSessionExt {
crates/oxvba-host/src/compat.rs:517:pub trait RuntimeValueCompatEngineExt {
crates/oxvba-vm/src/interpreter.rs:285:    pub fn snapshot(&self, slot_count: usize) -> Vec<RuntimeValue> {
crates/oxvba-vm/src/interpreter.rs:292:    pub fn snapshot_compat_values(&self, slot_count: usize) -> Vec<RuntimeValue> {
crates/oxvba-vm/src/interpreter.rs:305:    pub fn snapshot_values(&self, slot_count: usize) -> Vec<RuntimeValue> {
```

The three `oxvba-vm/src/interpreter.rs` entries above are guarded by
`#[cfg(test)]`; the production compatibility boundary for VM callers is
`oxvba_vm::compat`.

A targeted scan of normal host observation/frontend files shows no production
`RuntimeValue` usage outside test modules:

```text
rg -n "RuntimeValue" crates/oxvba-host/src/debugger.rs \
  crates/oxvba-host/src/immediate.rs crates/oxvba-host/src/embedded.rs \
  crates/oxvba-host/src/lib.rs crates/oxvba-cli/src/main.rs \
  crates/oxvba-web-host/src/lib.rs crates/oxvba-web-shell/src/lib.rs
# hits are test-module assertions/imports and compat-trait imports only
```

## Verification

All commands below passed on 2026-05-01 after the migration:

```text
cargo test -p oxvba-host --lib --tests
# ok: 926 host lib tests passed; host integration tests passed; environment-specific lanes ignored as before

cargo test -p oxvba-vm
# ok: 102 passed; 1 ignored

cargo check --workspace
# ok

cargo test -p oxvba-web-host -p oxvba-web-shell -p oxvba-cli
# ok: oxvba-cli 36 passed; oxvba-web-host 6 passed; oxvba-web-shell 5 passed
```

## Residuals / Next Owners

No blocker remains for `bd-pn5i.3`.

Residual `RuntimeValue` families remain intentionally open for later phase-2
beads:

- `bd-pn5i.4`: JIT compatibility snapshot and slot ABI surfaces.
- `bd-pn5i.5`: HAL, COM, runtime helper, and compatibility adapter boundaries.
- `bd-pn5i.8`: final RuntimeValue/fake-IR search gate and approved residual
  audit.

## Self-Review

- Normal VM/host execution-facing API names now prefer `Variant`.
- Legacy names still exist only when callers import `compat` traits/modules.
- UI/web/CLI paths no longer consume semantic RuntimeValue carriers as their
  normal DTOs; they consume variant observations and project display text.
- Tests were updated to make compatibility usage explicit where legacy
  RuntimeValue assertions remain.
