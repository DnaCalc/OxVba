# WORKSET_2026-03-05_COM_CLIENT_LATEBOUND_IMPLEMENTATION_V397_V400

## Scope

Execute the second C2 implementation block:
- `CreateObject` ProgID-text activation lane,
- member-name dispatch lane with deterministic cache semantics,
- invoke packing phase-I (`DispatchInvoke` 2-arg/3-arg subset),
- failure-path fixtures with `On Error Resume Next` evidence.

Profiles covered: `v397..v400`

## Deliverables

1. Compiler lowers known ProgID string literals for `CreateObject` to deterministic COM selector tokens.
2. Compiler lowers known member-name string literals for `DispatchInvoke` to deterministic member tokens.
3. `DispatchInvoke` supports a 2-arg property-get form in addition to existing 3-arg form.
4. HAL COM adapter maintains deterministic per-object member DISPID cache for known member-token lanes in native Windows mode.
5. HAL and host tests cover missing-argument normalization and stable failure routing semantics.
6. C2 fixture pack includes success and failure-path examples, including `On Error Resume Next` behavior on unsupported profiles.

## Verification Commands

- `cargo test -p oxvba-compiler compile_createobject_with_progid_literal_maps_to_known_token -- --nocapture`
- `cargo test -p oxvba-compiler compile_dispatchinvoke_accepts_two_arg_property_get_form -- --nocapture`
- `cargo test -p oxvba-hal windows_native_com_member_dispid_cache_populates_for_known_tokens -- --nocapture`
- `cargo test -p oxvba-host formal_v397_createobject_string_progid_subset_executes -- --nocapture`
- `cargo test -p oxvba-host formal_v400_string_com_lane_failure_routes_through_resume_next -- --nocapture`
- `./scripts/meta-check.ps1 -Fast`
