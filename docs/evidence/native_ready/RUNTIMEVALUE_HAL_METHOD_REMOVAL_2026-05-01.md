# RuntimeValue HAL Method Removal — 2026-05-01

Bead: `bd-0w46` / remove RuntimeValue type and bridges

## Scope

This checkpoint removes legacy `RuntimeValue` HAL method lanes where retained `Variant` companions already existed, then migrates active callers and HAL tests to the direct `Variant` APIs.

## Removed HAL lanes

- Console: `print_line`, `input_fields`, `line_input`
- UI: `msg_box`, `input_box`
- Event pump: `do_events`
- Time/locale: `date_serial_now`, `time_serial_now`, `timer_ticks`
- Diagnostics: `emit`, `debug_print`
- Process/environment: `shell`, `environ`, `dir`
- File system: `open`, `close`, `kill`, `seek`, `eof`, `lof`, `free_file`, `read_bytes`, `write_bytes`, `print_line`, `input_fields`, `line_input`, `loc`
- COM: `create_object`, `release_object`, `unsubscribe_event`, `event_callback_arg`, `release_event_callback`, `dispatch_invoke_runtime_value_v2`, `dispatch_invoke_dynamic_runtime_value_v2`
- Dynamic link: `bind_descriptor_value`, `prepare_invoke`, `invoke_bound`, `invoke_bound_multi`, `invoke_descriptor`, `invoke_descriptor_multi`, `invoke_symbol`

`ComHal::invalidate_typelib_cache` was converted in place to return `Variant` because it had no separate companion.

## Caller migration

- VM COM event cleanup/release now uses `unsubscribe_event_variant` and `release_event_callback_variant`.
- Host COM test helpers now use `create_object_variant` and `dispatch_invoke_variant`.
- HAL conformance probes now use `*_variant` dynamic-link and COM APIs.
- HAL adapter tests now build against `Variant` helpers instead of `RuntimeValue` HAL lanes.

## Validation

Commands run from repository root:

```text
cargo fmt --all
cargo check --workspace
cargo check --workspace --all-targets
rg -n "fn (print_line|input_fields|line_input|msg_box|input_box|do_events|date_serial_now|time_serial_now|timer_ticks|emit|debug_print|shell|environ|dir|open|close|kill|seek|eof|lof|free_file|read_bytes|write_bytes|loc|create_object|release_object|unsubscribe_event|event_callback_arg|release_event_callback|dispatch_invoke_runtime_value_v2|dispatch_invoke_dynamic_runtime_value_v2|invoke_bound|invoke_descriptor|invoke_symbol)\\b" crates/oxvba-hal/src --glob '*.rs'
rg -n "runtime_value_to_vba_str|runtime_value_to_vba_string|coerce::compat" crates --glob '*.rs'
```

Results:

- `cargo fmt --all`: passed.
- `cargo check --workspace`: passed.
- `cargo check --workspace --all-targets`: passed.
- Legacy HAL method signature search: no matches.
- Deleted runtime string coercion helper search: no matches.

## Follow-up HAL source clean gate

A follow-up cleanup routed Standard COM through `WindowsComBridge::dispatch_invoke_variant` / `dispatch_invoke_dynamic_variant` and deleted `oxvba-hal::compat`. The HAL crate source is now clean for both `RuntimeValue` and `runtime_value` mentions:

```text
rg -n "RuntimeValue|runtime_value" crates/oxvba-hal/src --glob '*.rs'
```

Result: no matches.

## Residuals

This checkpoint does not claim full `RuntimeValue` eradication. Remaining known residuals are outside HAL, including COM bridge internals that still project some Windows dispatch results through `RuntimeValue` before converting to retained `Variant`.
