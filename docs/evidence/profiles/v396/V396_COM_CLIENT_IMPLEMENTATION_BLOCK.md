# V396 COM Client C2 Implementation Block

## Scope
- Ladder: `v387..v406`
- Completed slice: `v393..v396`
- Workset: `WORKSET_2026-03-04_COM_CLIENT_LATEBOUND_IMPLEMENTATION_V393_V396.md`

## Outputs
- HAL COM adapter now keeps durable native dispatch bindings per object token in host-backed Windows mode.
- COM object-token identity/lifetime invariants are asserted and covered by adapter tests.
- Deterministic COM error mapping artifacts are published and adapter messages include stable labels for key HRESULT families.
- C2 conformance fixture scaffold is established under `conformance/com/client/c2-latebound/`.
- Meta-check now enforces terminal-gate sync across `AGENTS.md`, `docs/AUTORUN_STATE.md`, and readme guardrails.

## Gate Signal
- `v396` implementation block is complete; C2 runtime implementation lanes can proceed from a hardened bridge baseline.
