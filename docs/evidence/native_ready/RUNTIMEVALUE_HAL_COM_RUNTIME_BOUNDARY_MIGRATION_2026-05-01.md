# RuntimeValue HAL/COM/Runtime Boundary Migration Evidence

Date: 2026-05-01
Bead: `bd-pn5i.5` / `cleanout-004`
Workset: `WORKSET_2026-04-30_RUNTIMEVALUE_IR_STUB_CLEANOUT.md`

## Outcome

The HAL/COM/runtime compatibility slice narrowed `RuntimeValue` from a broad
root/runtime import into explicit compatibility boundaries:

- `oxvba_runtime` no longer re-exports `RuntimeValue` from the crate root.
  Compatibility callers now import `oxvba_runtime::compat::RuntimeValue`.
- Runtime string coercion helpers that accept/return `RuntimeValue` moved behind
  `oxvba_runtime::coerce::compat` and are re-exported through
  `oxvba_runtime::compat`.
- Runtime pointer helper APIs that accept/return `RuntimeValue` moved behind
  `oxvba_runtime::pointer_helpers::compat`; retained pointer helper APIs stay on
  `Variant`/SAFEARRAY-shaped carriers.
- The portable COM dispatch trait now carries retained `Variant` values. Legacy
  `RuntimeValue` projection is available only through
  `oxvba_com::platform::portable::compat::RuntimeValueCompatPortableDispatch`.
- HAL/COM/host/JIT/VM call sites that still consume legacy semantic carriers now
  import them from `oxvba_runtime::compat`, making compatibility use explicit.

## Key changed surfaces

- Added `crates/oxvba-runtime/src/compat.rs` as the named runtime compatibility
  module.
- Updated `crates/oxvba-runtime/src/lib.rs` so the normal root exports are
  retained substrate types (`Variant`, `VarType`, `ObjectRef`, numeric helper
  types) and not `RuntimeValue`.
- Moved runtime-value string coercion helpers under
  `crates/oxvba-runtime/src/coerce.rs::compat`.
- Moved runtime-value pointer helper wrappers under
  `crates/oxvba-runtime/src/pointer_helpers.rs::compat`.
- Updated `crates/oxvba-com/src/platform/portable.rs` so `PortableDispatch`
  uses `Variant`; compatibility projection is a named extension trait.
- Updated imports across HAL/COM/host/JIT/VM/launcher/language-service/tests
  from the old root `oxvba_runtime::RuntimeValue` shape to explicit
  `oxvba_runtime::compat::RuntimeValue`.

## Search evidence

Commands run after migration:

```text
rg -n "pub use runtime_value::\{[^}]*RuntimeValue|pub use crate::runtime_value::RuntimeValue" crates/oxvba-runtime/src/lib.rs crates/oxvba-runtime/src/compat.rs
```

Result: only `crates/oxvba-runtime/src/compat.rs` re-exports `RuntimeValue`.

```text
rg -n "oxvba_runtime::RuntimeValue|use oxvba_runtime::RuntimeValue" crates --glob '*.rs'
```

Result: no old root-import call sites remain; legacy call sites use
`oxvba_runtime::compat::RuntimeValue`.

```text
rg -n "RuntimeValue" crates/oxvba-runtime/src crates/oxvba-hal/src crates/oxvba-com/src --glob '*.rs' | wc -l
```

Result after this bead: `793`. The count is not expected to be zero in this
slice because HAL trait compatibility lanes, COM model bridge helpers, Variant
bridge helpers, SAFEARRAY legacy constructors, and tests remain explicit
compatibility/residual surfaces for the final search gate.

## Verification

Passed:

```text
cargo fmt --all
cargo check --workspace
cargo test -p oxvba-runtime -p oxvba-com -p oxvba-hal
cargo test -p oxvba-host --tests
cargo test -p oxvba-vm -p oxvba-jit
```

## Approved residual and removal path

Residual `RuntimeValue` families after this bead are explicitly compatibility or
test surfaces, not normal root runtime imports:

1. HAL trait legacy methods still accept/return `RuntimeValue`, with retained
   `_variant` companions implemented directly by adapters. Removal path:
   split the legacy HAL methods into a dedicated `oxvba_hal::compat` extension
   trait after all VM/JIT/host callers are verified on the variant companions.
2. COM model and dynamic-object `from_runtime_value` / `to_runtime_value`
   bridge methods remain for compatibility tests and older callers. Removal
   path: move them behind `oxvba_com::compat` extension traits during the final
   RuntimeValue search-gate bead if no external public-API blocker is accepted.
3. Runtime `Variant`/`SafeArray` bridge helpers remain as compatibility
   projection helpers. Removal path: migrate direct tests and any remaining
   compatibility callers to explicit `oxvba_runtime::compat` traits/modules,
   then remove inherent RuntimeValue bridge methods or document a public API
   blocker in `cleanout-007`.
4. Presentation/front-end surfaces are handled by `cleanout-005`; this bead only
   changed their imports to the explicit runtime compatibility module when they
   still consume legacy values.

These residuals are intentionally carried forward to `cleanout-007` search-gate
verification and are not a claim that the full umbrella `RuntimeValue` gate is
complete.
