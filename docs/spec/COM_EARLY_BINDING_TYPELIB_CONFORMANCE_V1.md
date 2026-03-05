# COM Early Binding and Type Library Conformance V1

Status: `working-draft`
Date: 2026-03-05
Companion scope: `docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md`

Implementation snapshot (`v417..v426`):
- E1 subset is active via PMR resolver tests covering deterministic libid/importlib resolution and ambiguity handling.
- E2 subset is active via compiler project-lowering tests for constrained external declarations and member rewrite diagnostics.
- E4 substrate is active in HAL via deterministic metadata cache/invalidation operations (initial known-identity subset).
- E3 runtime currently executes through existing late-bound COM transport for the constrained rewritten subset; dedicated native early-bound runtime lanes remain staged for later profiles.

## 1. Goal

Define executable verification lanes for early-bound COM + type-library support with clause-level traceability and deterministic evidence capture.

## 2. Lane matrix

### Lane E0: Structural metadata ingest

Checks:

1. load/normalize typelib metadata into OxVba internal descriptor graph,
2. validate required flags and descriptor consistency (`TYPEATTR`, `FUNCDESC`, `VARDESC`, dual flags),
3. deterministic normalization/fingerprint output.

Evidence:

- `docs/evidence/conformance/com_early/lanes/E0_*.csv`

### Lane E1: PMR reference resolution (typelib-focused)

Checks:

1. importlib/libid/version resolution,
2. ambiguous/missing/broken deterministic diagnostics,
3. precedence interactions with project references.

Evidence:

- `docs/evidence/conformance/com_early/lanes/E1_*.csv`

### Lane E2: Binder/type resolution

Checks:

1. declared external type resolution (`Dim x As MyLib.MyThing`),
2. `As New` auto-instantiation eligibility checks,
3. early-bound member resolution and overload selection subset.

Evidence:

- compile-time diagnostic snapshots,
- resolved-member trace artifacts.

### Lane E3: Runtime early-bound invoke

Checks:

1. early-bound method/property execution against controlled test COM server,
2. deterministic runtime errors for mismatch/unsupported paths,
3. dual-interface strategy behavior under configured policy.

Evidence:

- runtime result snapshots,
- diagnostic maps with HRESULT roots.

### Lane E4: Cache and invalidation

Checks:

1. cache hit/miss determinism,
2. invalidation on reference mutation and fingerprint drift,
3. no stale metadata use after invalidation event.

Evidence:

- cache timeline traces,
- deterministic replay comparison output.

### Lane E5: End-to-end project integration

Checks:

1. project manifest with typelib references compiles/runs end to end,
2. reference precedence and name-shadow behavior with external types,
3. mixed early-bound and late-bound codepaths in one project.

Evidence:

- integrated run logs and fixtures.

### Lane E6: Formal/property lanes (deferred-gate non-blocking)

Checks:

1. Kani proofs for reduced-state binding determinism and transition safety,
2. property tests for cache and diagnostic totality,
3. unsafe boundary checks once vtable lane is introduced.

Evidence:

- formal lane csv rows,
- deferred gate entries if timeout/resource limits occur.

## 3. Source-clause mapping baseline

### MS-VBAL anchors

- `SPEC-...-01489`, `...-01497`, `...-01498` -> `As New` type legality and instantiation constraints.
- `SPEC-...-01229`, `...-01230` -> project reference ordering and precedence.
- `SPEC-...-05318` -> CreateObject baseline relationship to object creation semantics.

### MS-OAUT anchors

- `CONF-...-0123`, `...-0125` -> dual and automation interface flags.
- `CONF-...-0708..0718` -> `ITypeComp::Bind` obligations.
- `CONF-...-0851` -> interface inheritance/implementation discovery.
- `CONF-...-1023`, `...-1024` -> type lookup by GUID/name.
- `CONF-...-0080..0084` (existing) -> invoke output channels.

### MS-OVBA / host operational anchors

- `ProjectReferences` and reference record structures for persisted source of truth.
- VBA references APIs (`AddFromGuid`, `AddFromFile`, `IsBroken`) for oracle and harness construction.

## 4. Controlled fixture plan

Fixture roots:

- `conformance/com/early/typelib_ingest/`
- `conformance/com/early/binder/`
- `conformance/com/early/runtime/`
- `conformance/com/early/cache/`
- `conformance/com/early/end_to_end/`

Core fixture families:

1. simple dual interface (`Count`, `Exists`, scalar args),
2. optional/default/byref stress fixture,
3. versioned typelib mutation fixture,
4. ambiguous symbol fixture across refs.

## 5. Conformance scripts (planned)

Primary orchestration:

- `scripts/run-com-early-conformance.ps1`

Lane scripts:

- `scripts/run-com-early-lane-e0.ps1`
- `scripts/run-com-early-lane-e1.ps1`
- `scripts/run-com-early-lane-e2.ps1`
- `scripts/run-com-early-lane-e3.ps1`
- `scripts/run-com-early-lane-e4.ps1`
- `scripts/run-com-early-lane-e5.ps1`
- `scripts/run-com-early-lane-e6-formal.ps1`

## 6. Artifact schema

Required output columns:

- `lane_id`
- `test_id`
- `profile`
- `runtime_class`
- `clause_ids`
- `status` (`pass|fail|skip|deferred`)
- `diagnostic_code`
- `hresult` (optional)
- `evidence_path`
- `repro_command`

Latest pointers:

- `docs/evidence/conformance/com_early/COM_EARLY_CONFORMANCE_LATEST.csv`
- `docs/evidence/conformance/com_early/COM_EARLY_CONFORMANCE_LATEST.md`

## 7. Deferred oracle topics

Track as explicit deferred items until parity runs are captured:

1. exact Office-host behavior for broken-reference repair prompts and timing,
2. exact dual-interface fallback behavior under mixed server implementations,
3. typelib version selection parity in edge cases.

Target tracker:

- `docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv`

## 8. Exit criteria for initial implementation closure

1. E0..E5 deterministic lanes implemented and passing in CI-suitable environment.
2. E6 formal/property lanes running with documented status (pass/timeout/deferred).
3. Clause mapping coverage table has no unowned required clauses for the implemented subset.
4. All unsupported/deferred behaviors are explicit in implementation-defined registers.
