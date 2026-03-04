# PROFILE_LADDER_2026-03-04_MACH1000_V337_V366_COM_SERVER_DEPTH

## Range

- Ladder span: `v337..v366`
- Focus: COM server scaffold and host-integration hooks (Windows-first, deterministic policy controls).

## Steps

| Step | Focus | Deliverables |
|---|---|---|
| `v337` | server scope lock | server-lane objective and boundaries lock |
| `v338` | server policy controls | explicit enable/deny policy surface |
| `v339` | server lifecycle model | class/object lifetime and shutdown contract |
| `v340` | minimal class registration API | scaffold interfaces for server publication |
| `v341` | class factory shell | class-factory object skeleton |
| `v342` | dispatch shell | minimal `IDispatch` façade scaffold |
| `v343` | method table model | deterministic method-token routing contract |
| `v344` | argument decode scaffold | variant decode precondition checks |
| `v345` | return encode scaffold | variant encode postcondition checks |
| `v346` | host bridge adapter hooks | host->server wiring extension points |
| `v347` | server diagnostics mapping | deterministic server-side fault mapping |
| `v348` | activation harness skeleton | local harness for activation/invoke probes |
| `v349` | policy-denied harness cases | deterministic denial assertions |
| `v350` | unsupported-profile harness cases | deterministic unsupported assertions |
| `v351` | server clause additions | clause catalog + verification mapping updates |
| `v352` | server conformance lane draft | L3 lane executable plan update |
| `v353` | server conformance script scaffold | script orchestration entrypoints |
| `v354` | host integration doc pass | runtime/host/hal interaction notes |
| `v355` | safety assertions pass | pre/post/invariant assertions for scaffold |
| `v356` | stability tests I | regression checks for existing runtime paths |
| `v357` | stability tests II | regression checks for host-backed capabilities |
| `v358` | formal lane hooks | model-checking topics for server lifecycle |
| `v359` | deferred-gate sync | deferred register and backlog updates |
| `v360` | evidence packet I | server harness output capture |
| `v361` | evidence packet II | summarized findings + caveats |
| `v362` | docs/index sync | docs/spec/readme crosslinks |
| `v363` | profile status prep | statuses/artifacts staged |
| `v364` | integrated gate prep | gate command plan + expected outputs |
| `v365` | integrated gate | `v337..v366` server-depth integrated check |
| `v366` | terminal closure | closure record and handoff to stabilization ladder |
