# COM Default Member Expression Context Evidence

Date: 2026-07-02
Bead: `bd-aprs.8.8.6` under `bd-aprs.8.8`
Worksets:
- `docs/worksets/WORKSET_2026-05-31_FRONTEND_TOKENIZER_PARSER_BINDER_AST_REFACTOR.md`
- `docs/worksets/WORKSET_2026-03-19_IP-08B_EXECUTION_CHECKLIST.md`

## Outcome

Binary expression binding now applies the same default-member value context that assignment already
used for object values.

For typed imported COM receivers and host-injected COM receivers, object operands in arithmetic,
concatenation, and other ordinary binary value contexts now bind through the receiver's descriptor
backed default `PropertyGet` instead of leaving the raw object as the operand. The `Is` identity
operator stays object-reference based and does not default either side.

## Regression Shape

- `r = w + w2` where `w` and `w2` are typed COM objects with a default scalar property emits two
  descriptor-backed `EarlyCom` default `PropertyGet` calls before the binary add.
- `s = "x=" & w` emits one descriptor-backed default `PropertyGet` before string concatenation.
- The same expression shape is covered for imported typelib receivers and host-injected receivers.
- `If w Is w2 Then ...` remains valid object-identity syntax in the same fixture, proving the
  identity operator does not consume default members.

## Checks

- `cargo test -p oxvba-bind --test bind_roundtrip default_member_binary_expression -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip typed_com_default_member -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip host_injected_default_member -- --nocapture`
- `cargo test -p oxvba-bind --test bind_roundtrip is_operator -- --nocapture`
- `cargo clippy -p oxvba-bind --tests -- -D warnings`
- `cargo test -p oxvba-bind -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`
- `br dep cycles --json`
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check-governance.ps1`

## Boundary

This closes the typed COM default-member binary expression-context slice for frontend binding. It
does not close the full Office object model, live COM runtime dispatch matrix, ParamArray/ByRef-out
COM method rows, or every imported member/property/default-member row under `bd-aprs.8.8`/`IP-08B`.

No Excel oracle run was needed for this slice; the behavior target is the ordinary VBA expression
rule that object values in value contexts use their default member, while `Is` remains identity.
