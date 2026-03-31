# WORKSET_2026-03-31_CLI_PIT_OF_SUCCESS_AND_REFERENCE_OVERRIDES

Status: `planned`
Date: 2026-03-31
Owner: OxVBA CLI / project system / host bootstrap

## Purpose

Make OxVBA feel obvious and easy for the first user while preserving the existing internal host/profile/policy flexibility needed by embedded and advanced scenarios.

The pit of success for this area is:
- `oxvba run hello.bas` works on a normal local machine with no extra flags.
- `oxvba run-project .` works for small convention-mode projects with no `.basproj`.
- `.basproj` becomes necessary when the user needs durable metadata such as references, explicit startup, or stable build settings.
- restrictive or unusual runtime/policy behavior remains deterministic, but is explicit rather than surprising.

## Principles

1. Deterministic does not require restrictive defaults.
2. Local CLI defaults should optimize for successful first-run experience.
3. Embedded-host flexibility remains first-class, but should not leak into the simplest user path.
4. `OutputType` remains semantic; `BuildTarget` remains packaging/build shape.
5. CLI overrides must have a simple, explicit precedence model.

## Scope

This workset covers:
- CLI default runtime/profile/policy pit-of-success repair
- precedence rules for CLI options vs project metadata vs discovery defaults
- small ad hoc CLI reference injection for ephemeral runs
- effective-config explainability surface
- convention-to-project upgrade path

This workset explicitly notes, but does not require in the first closure slice:
- COM reference helper UX such as ProgID-driven reference selection, typelib suggestion/listing, and repair flows

## Desired User Model

### Smallest useful path

```powershell
oxvba run hello.bas
```

Expected behavior:
- source compiles
- `Print` writes to console on local stdio profiles
- no policy-denied surprise for ordinary local use

### Slightly larger path

```powershell
oxvba run-project .\my-tool
```

Expected behavior:
- convention-mode discovery
- deterministic startup ladder
- no `.basproj` required unless project metadata is actually needed

### Formal project path

Use `.basproj` when the user needs:
- ordered references
- explicit startup
- explicit output semantics
- stable build configuration
- host/policy defaults

## Precedence Contract

### Scalar settings

Precedence order:
1. CLI option
2. project metadata (`.basproj` / imported `.vbp`)
3. discovery/platform default

Examples:
- profile
- policy preset
- entrypoint
- build target

### Collection settings

CLI additions are additive by default.

Rules:
- project-declared reference order is preserved
- CLI-added references append in CLI order
- exact-identity duplicate means CLI replaces that item in-place
- ambiguous conflict means deterministic error

This applies to:
- project references
- COM/type library references
- native references

## Planned Execution Slices

### Slice A: first-run repair

- Change default local CLI execution to a practical local-dev lane.
- Keep deterministic profile discovery.
- Ensure `Print` works on the default local stdio path.
- Tighten README/help text to match the actual default behavior.

### Slice B: precedence formalization

- Publish an explicit precedence section in README + spec.
- Add executable coverage for CLI/project/default interactions.

### Slice C: ad hoc reference injection

Add a bounded convenience surface for ephemeral runs:
- `--project-ref <path>`
- `--com-ref <lib-or-name>`
- `--native-ref <path>`

This is convenience-only. Durable reference truth still belongs in `.basproj`.

### Slice D: explainability surface

Add an effective-config inspection surface, likely via:
- `oxvba explain [PATH]`
or
- an expansion of `oxvba host-check [PATH]`

It should show:
- discovered project lane
- startup choice
- effective runtime profile
- effective policy preset and overrides
- effective references in order
- build-target/output interpretation when relevant

### Slice E: convention upgrade

Add:
- `oxvba init --from-convention <dir>`

This should write a `.basproj` that captures what convention discovery currently sees, so users can graduate from informal mode to stable project metadata without manual transcription.

## Adjacent COM UX Note

Not required for first closure, but should be tracked explicitly:

OxVBA likely needs a helper-oriented COM reference UX, for example:
- infer or repair a COM reference from a ProgID
- list plausible typelib/library options for a ProgID or library name
- show candidate matches and the canonical `.basproj` form to persist

This should remain a helper/advisor layer, not hidden magic. The durable result should still be an explicit `.basproj` `COMReference`.

## Proposed Bead Breakdown

- `bd-cli2.1` local CLI default pit-of-success repair
- `bd-cli2.2` precedence contract docs + executable coverage
- `bd-cli2.3` bounded ad hoc CLI reference injection
- `bd-cli2.4` effective-config explain surface
- `bd-cli2.5` convention-to-project upgrade command
- `bd-cli2.6` COM reference helper UX planning and bounded helper implementation

## Closure Shape

This workset is closure-ready when:
- first-run local CLI behavior works with no surprise policy denial
- precedence rules are documented and tested
- ad hoc references exist in a bounded form
- users can inspect effective execution/reference configuration
- convention projects can be upgraded into `.basproj`
- the COM helper lane is at least planned explicitly, even if intentionally sequenced after the core slices
