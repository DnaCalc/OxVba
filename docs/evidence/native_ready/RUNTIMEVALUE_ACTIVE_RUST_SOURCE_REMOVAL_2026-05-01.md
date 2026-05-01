# RuntimeValue Active Rust Source Removal (2026-05-01)

Status: evidence for bead `bd-0w46` / remove `RuntimeValue` type and bridges.

## Outcome

Active Rust source no longer carries `RuntimeValue` as a type, bridge, or compatibility API. Retained execution and boundary carriers are `Variant` and SAFEARRAY `Variant` payloads.

Removed or migrated surfaces:

- deleted `crates/oxvba-runtime/src/runtime_value.rs` and moved surviving scalar/helper types to `crates/oxvba-runtime/src/value_types.rs`;
- deleted runtime and host compatibility modules that projected to/from `RuntimeValue`;
- removed `Variant`/`RuntimeValue` bridge helpers and legacy SAFEARRAY value APIs;
- removed VM/JIT slot/snapshot compatibility shims that exposed `RuntimeValue`;
- removed host engine compatibility snapshots/invocation helpers that exposed `RuntimeValue`;
- updated remaining native-pointer/native-declare host tests to assert direct retained `Variant` carriers, including exact `Byte`/`Integer` carrier expectations;
- made `coerce_to(..., VarType::String)` return a BSTR-backed `Variant` instead of a legacy-side error path.

## Validation

Commands run from repo root:

```bash
cargo fmt --all
cargo check --workspace --all-targets
cargo test --workspace
rg -n "RuntimeValue|runtime_value" crates --glob '*.rs'; test $? -eq 1
```

Results:

- `cargo fmt --all`: passed.
- `cargo check --workspace --all-targets`: passed.
- `cargo test --workspace`: passed.
- `rg -n "RuntimeValue|runtime_value" crates --glob '*.rs'; test $? -eq 1`: passed with no active Rust source matches.

Targeted checks also run while fixing direct carrier assertions:

```bash
cargo test -p oxvba-host --test native_declare_string_marshalling_end_to_end
cargo test -p oxvba-host --test pointer_helpers_end_to_end
cargo test -p oxvba-runtime --lib
```

All targeted checks passed.

## Coverage Notes

Several old `#[cfg(test)]` modules existed only to exercise `RuntimeValue` compatibility APIs. Those compatibility APIs were removed instead of preserved. Replacement coverage is now carried by direct `Variant` tests that remain active, including:

- runtime SAFEARRAY and pointer helper unit tests;
- JIT slot ABI `Variant` carrier tests;
- VM snapshot API `Variant` tests;
- host native declare and pointer helper end-to-end tests;
- full workspace test coverage above.

Historical docs and evidence may continue to mention `RuntimeValue`; the active Rust source search gate is intentionally scoped to `crates/**/*.rs`.
