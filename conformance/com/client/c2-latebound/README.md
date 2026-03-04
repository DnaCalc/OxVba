# C2 Late-Bound COM Client Fixtures

Status: `active-subset` (`v400`)

This directory stores the first executable fixture subset for C2 late-bound COM client behavior.

Current fixtures:
- `createobject_string_prog_id_success.bas`: string ProgID activation lowering subset.
- `dispatch_member_name_success.bas`: member-name invoke lowering with explicit argument.
- `dispatch_member_name_two_arg_property_get.bas`: 2-arg property-get lowering subset.
- `dispatch_member_name_failure_resume_next.bas`: failure-route harness for `On Error Resume Next`.

Planned next fixture families:
- `dispatch_named_optional_args_*`
- `dispatch_argerr_excepinfo_*`

Execution note:
- These fixtures are currently asserted via host formal tests and profile evidence.
- Lane wiring to the integrated conformance runner is tracked in `docs/spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md` (`Lane L2b`).
