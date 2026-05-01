# RuntimeValue JIT Surface Migration Evidence

Date: 2026-05-01
Bead: `bd-pn5i.4` / `cleanout-003 migrate JIT snapshot and slot ABI surfaces`
Workset: `docs/worksets/WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md`

## Outcome

JIT snapshot, Cranelift wrapper, JIT context, and slot ABI surfaces now expose
retained `Variant` carriers as the normal API. Legacy semantic projections are
available only through explicit compatibility boundaries.

## Surface Changes

- `JitEngine` retained normal APIs are `execute_and_snapshot_variants` and
  `execute_and_snapshot_variants_with_host`.
- Legacy JIT snapshot methods were removed from the inherent `JitEngine` API and
  are now available through `oxvba_jit::compat::RuntimeValueCompatJitEngineExt`
  or the existing `oxvba_jit::compat::*` free functions.
- Cranelift RuntimeValue execution wrappers were removed; normal Cranelift
  wrappers are `execute_bytecode_variants` and `execute_bytecode_rtslot_variants`.
- `RtSlot` normal API is Variant-shaped (`variant`, `from_variant`, `vtype`,
  payload helpers). RuntimeValue slot construction/projection moved to
  `oxvba_jit::slot_abi::compat::RuntimeValueCompatRtSlotExt` and
  `rtslot_from_runtime_value`.
- `JitContextOwned` and `JitContext` normal APIs use `extract_user_variants`,
  `read_variant_slot`, and `write_variant_slot`. RuntimeValue extraction/read/write
  moved to `oxvba_jit::jit_context::compat` extension traits.
- JIT tests import the relevant compatibility traits explicitly where legacy
  RuntimeValue assertions remain.

## Scan Evidence

Commands run after migration:

```text
rg -n "RuntimeValue" crates/oxvba-jit/src --glob '*.rs' | wc -l
# 200

rg -n "pub (struct|enum|trait|fn)|pub fn" crates/oxvba-jit/src --glob '*.rs' \
  | rg "RuntimeValue|execute_and_snapshot\\(|execute_bytecode\\(|read_slot\\(|write_slot|extract_user_values|rtslot_from_runtime_value|try_to_runtime_value|to_runtime_value"
```

The public-surface scan now returns only explicit JIT compatibility boundaries:

```text
crates/oxvba-jit/src/slot_abi.rs:80:    pub trait RuntimeValueCompatRtSlotExt {
crates/oxvba-jit/src/slot_abi.rs:119:    pub fn rtslot_from_runtime_value(value: &RuntimeValue) -> RtSlot {
crates/oxvba-jit/src/lib.rs:87:    pub trait RuntimeValueCompatJitEngineExt {
crates/oxvba-jit/src/lib.rs:158:    pub fn execute_and_snapshot(
crates/oxvba-jit/src/jit_context.rs:285:    pub trait RuntimeValueCompatJitContextOwnedExt {
crates/oxvba-jit/src/jit_context.rs:302:    pub trait RuntimeValueCompatJitContextExt {
```

The `lib.rs:158` entry is inside `oxvba_jit::compat`; there are no normal
inherent/public JIT APIs returning `RuntimeValue` after this migration.

## Verification

All commands below passed on 2026-05-01 after the migration:

```text
cargo test -p oxvba-jit
# ok: 60 passed; 0 failed

cargo check --workspace
# ok
```

## Residuals / Next Owners

No blocker remains for `bd-pn5i.4`.

Residual RuntimeValue families remain intentionally open for later phase-2 beads:

- `bd-pn5i.5`: HAL, COM, runtime helper, enum/re-export, and compatibility
  adapter boundaries.
- `bd-pn5i.6`: launcher/web/language-service presentation DTOs.
- `bd-pn5i.8`: final RuntimeValue/fake-IR search gate and approved residual
  audit.

## Self-Review

- JIT execution-facing APIs now require retained `Variant` unless callers opt
  into `compat`.
- Slot ABI and JIT context compatibility paths are named with `RuntimeValueCompat*`.
- JIT tests remain behaviorally equivalent and make compatibility use explicit.
