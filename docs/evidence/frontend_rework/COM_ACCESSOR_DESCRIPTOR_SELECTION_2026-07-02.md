# COM Accessor Descriptor Selection Evidence

Date: 2026-07-02
Bead: `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Outcome

Imported and host-injected COM member/default-member binding now selects descriptors by the
requested accessor instead of by typelib row order.

Read contexts (`want = None`) choose only read-callable descriptors (`PropertyGet`, then `Method`)
and never carry a write-only `PropertyPut`/`PropertyPutRef` descriptor into `CoreCallee::EarlyCom`.
Write contexts require the requested `PropertyLet`/`PropertySet` descriptor. Bare/default-member
assignment paths now ask the provider for the write accessor up front.

The same pass removed a related permissive route: statically known COM, host-injected COM, and
referenced-project object types no longer fall back to late binding when a member or requested
accessor is absent. Plain `Object` and `Variant` late-binding behavior is unchanged.

## Regression Shape

- A synthetic typelib with a default `PropertyPut` row before the matching `PropertyGet` row now
  resolves read/default-member access to the getter and write access to the putter.
- Binder tests assert the canonical COM descriptor's `invoke_kind`, not only the call-site
  `ProjectMemberKind`, so OxIR/vm3 cannot inherit a mismatched descriptor.
- A cross-project get-only property assignment now fails binding instead of silently dispatching a
  late `PropertyLet` through the getter/member name.

## Checks

- `cargo test -p oxvba-symbol -- --nocapture`
- `cargo test -p oxvba-bind -- --nocapture`
- Focused checks before the full package runs:
  - `cargo test -p oxvba-symbol com_default_member_accessor_selection_ignores_typelib_order -- --nocapture`
  - `cargo test -p oxvba-bind --test bind_roundtrip typed_com_default_member_put_before_get_preserves_accessor_descriptors -- --nocapture`
  - `cargo test -p oxvba-bind --test cross_project cross_project_property_let_does_not_fallback_to_getter -- --nocapture`

## Boundary

This is descriptor-selection and missing-accessor parity hardening. It does not close the broader
IP-08B host-returned COM matrix or all FE-7.6.a imported COM activation/member breadth. Those lanes
remain `in-progress` until the scoped host/imported COM matrices are explicit and proved.
