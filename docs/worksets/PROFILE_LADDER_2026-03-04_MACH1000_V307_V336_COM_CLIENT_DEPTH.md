# PROFILE_LADDER_2026-03-04_MACH1000_V307_V336_COM_CLIENT_DEPTH

## Range

- Ladder span: `v307..v336`
- Focus: native Windows COM client depth with deterministic contracts and projection fallback safety.

## Steps

| Step | Focus | Deliverables |
|---|---|---|
| `v307` | client baseline lock | source/crosswalk lock for client-only lane |
| `v308` | token->ProgID policy | explicit mapping contract for tokenized COM boundary |
| `v309` | activation fallback policy | native/fallback precedence rule and diagnostics |
| `v310` | apartment guard formalization | COM init/uninit contract for host-backed calls |
| `v311` | native activation helper | reusable activation utility layer |
| `v312` | dispatch pointer lifecycle | release safety and deterministic cleanup |
| `v313` | member-resolution v1 | mapped member token rules for first automation target |
| `v314` | invoke-noarg lane | `IDispatch::Invoke` property-get baseline |
| `v315` | invoke-arg lane | one-arg method invoke baseline |
| `v316` | variant return mapping v1 | `VT_EMPTY`/`VT_I4`/`VT_UI4`/`VT_BOOL` token conversion |
| `v317` | HRESULT mapping notes | deterministic adapter-fault surface for invoke failures |
| `v318` | fallback compatibility checks | projection compatibility for unmapped/unavailable paths |
| `v319` | host-backed unit checks I | native activation/invoke unit tests |
| `v320` | host-backed unit checks II | dictionary/member mapping contract tests |
| `v321` | conformance clause uplift | add host-tested COM native clause coverage |
| `v322` | non-Windows contract recheck | deterministic unsupported shape unchanged |
| `v323` | runtime policy sweep | strict/interactive compile/runtime mode checks |
| `v324` | VM boundary safety audit | ensure tokenized VM boundary still deterministic |
| `v325` | diagnostics wording pass | stable/clear error payload content |
| `v326` | client evidence bundle I | intermediate evidence report |
| `v327` | client evidence bundle II | repeatable command set + artifacts |
| `v328` | docs crosslink sync | spec/workset/implementation-log alignment |
| `v329` | deferred oracle updates | add client-lane unresolved parity topics |
| `v330` | formal lane hook | add formal obligations for client subset |
| `v331` | formal async kickoff | deferred-gate entries for extended model checks |
| `v332` | client smoke script scaffold | script entrypoint for COM client checks |
| `v333` | client smoke run | executable smoke evidence captured |
| `v334` | gate prep | artifacts and profile statuses for closure |
| `v335` | integrated gate | `v307..v336` client-depth integrated check |
| `v336` | terminal closure | closure record and handoff to server-depth ladder |
