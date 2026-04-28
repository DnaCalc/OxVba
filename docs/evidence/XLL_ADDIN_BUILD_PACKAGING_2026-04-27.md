# XLL Addin Build Packaging

Date: 2026-04-27
Bead: `bd-xll1.7`

## Scope

Give `.basproj` `OutputType=Addin` a concrete `oxvba build` package result
instead of silently producing only the canonical `.oxb` bundle.

## Changes

- `oxvba build` now defaults Addin projects to `<ProjectName>.xll`.
- The build command still compiles the canonical bundle first, validates native
  export descriptors, then stages the bundle into generated XLL shim source.
- The generated shim is compiled through `ShimOutputType::Xll`, producing the
  requested `.xll` artifact.
- Non-Addin project kinds keep their existing default `.oxb` output.

## Validation

```powershell
cargo test -p oxvba-cli default_build_output_path_uses_xll_for_addin_projects --quiet
cargo test -p oxvba-cli build_addin_project_produces_xll_artifact --quiet
```

Results:

- both commands pass
- the Addin build regression creates an Addin `.basproj`, runs `oxvba build`,
  verifies a non-empty `ExcelAddin.xll`, and verifies that the old default
  `ExcelAddin.oxb` output was not emitted

## Remaining Boundary

This proves local package emission. Excel-loaded registration and worksheet
invocation remain under the blocked host-validation bead.
