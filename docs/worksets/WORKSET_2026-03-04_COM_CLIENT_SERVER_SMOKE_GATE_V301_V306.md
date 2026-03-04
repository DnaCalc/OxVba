# WORKSET_2026-03-04_COM_CLIENT_SERVER_SMOKE_GATE_V301_V306.md

## Objective

Execute `v301..v306`: complete first integrated Windows COM smoke gate (client + server) with deterministic diagnostics and evidence output.

## Scope

1. HRESULT/EXCEPINFO/ArgErr deterministic mapping baseline.
2. COM server class factory + minimal dispatch surface.
3. Host policy integration for COM client/server enablement.
4. End-to-end smoke lane execution and evidence publication.
5. Terminal ladder closure and handoff prep.

## Deliverables

- COM error mapping contract and implementation.
- runnable COM server smoke scaffold.
- policy wiring and integration tests.
- conformance evidence bundle for `v305`.
- closure docs/profile status for `v306`.

## Checks

- smoke lanes L0-L4 pass for in-scope subset.
- deterministic error routing through VM/host diagnostics is verified.
- non-Windows unsupported behavior remains unchanged and explicit.

## Closure Conditions

`v306` is complete when the first COM series gate is passed with published evidence and the next ladder (`v307..v336`) can start from an executable baseline.

