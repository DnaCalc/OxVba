# C2 Late-Bound COM Client Fixture Scaffold

Status: `scaffold`

This directory reserves fixture space for the C2 late-bound COM client lane.

Planned fixture families:
- `createobject_string_prog_id_*`
- `dispatch_member_name_*`
- `dispatch_named_optional_args_*`
- `dispatch_argerr_excepinfo_*`

Execution note:
- These fixtures are scaffold-only in `v396` and are not yet wired into the default `conformance/tests` runner.
- Lane wiring is tracked in `docs/spec/COM_CLIENT_SERVER_CONFORMANCE_V1.md` (`Lane L2b`).
