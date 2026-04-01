# OxVBA v0.1.1

This release refreshes the Windows x64 binaries and tightens the public onboarding and host-integration story around the current OxVBA surface.

Artifacts:
- `oxvba-v0.1.1-windows-x64.zip`
  - `oxvba.exe`: the main OxVBA CLI
  - `oxvba-run.exe`: launcher for compiled `.oxb` bundles
  - `README.txt`: short local usage note

Quick start:

```powershell
oxvba.exe run hello.bas
oxvba.exe init .\demo-app
oxvba.exe run-project .\demo-app
```

Highlights:
- The README now has a cleaner first-time-user path:
  - start with `run file.bas`
  - then move to `init` + `run-project`
  - advanced runtime/profile/reference overrides are pushed later in the guide
- OxVBA now documents and exposes a stronger direct-host story for special-purpose IDEs such as OxIde:
  - `oxvba_languageservice::HostWorkspaceSession`
  - `oxvba_project::inspect_workspace_target`
- OxVBA now has typed COM reference-selection helpers on the direct host/API surface:
  - registered-library discovery by friendly/library name
  - ProgID lookup
  - file-backed typelib discovery from `.tlb`, `.olb`, `.dll`, `.ocx`, `.exe`, and `.xll` when an embedded typelib is actually present
  - active project reference assessment
  - deterministic add/replace/repair/remove edit planning for canonical `.basproj` COM references
- The direct COM selection surface is intended for OxIde and future richer CLI/project-edit flows; durable project truth still remains the ordered `.basproj` `<COMReference>` list.
- `oxvba.exe explain` / `oxvba.exe host-check` continue to show the effective discovered lane, startup, runtime/policy selection, and reference order for a target.
- `oxvba.exe run-project` / `build` continue to accept bounded ad hoc reference injection via `--project-ref`, `--com-ref`, and `--native-ref` for experiment-time overrides.
- `oxvba.exe init --from-convention <dir>` continues to upgrade a convention-mode directory into an explicit `.basproj`.
- `.oxb` remains the stable compiled OxVBA bundle format.
- `OutputType=Exe` still means executable startup semantics for OxVBA projects; the stable build artifact emitted by the CLI today is still `.oxb`, not a project-specific native Windows `.exe`.

Notes:
- These release binaries are intended to run directly on Windows x64 with no Rust toolchain installed.
- Local CLI execution still defaults to a practical Windows stdio lane, so `oxvba.exe run hello.bas` works without extra policy flags.
- The richer COM helper surface is currently strongest on the direct Rust API/host side; the richer CLI add/list/repair flow is the next step rather than a claim of current CLI parity.

For the full user guide, language-service host boundary, direct host session contract, and COM helper shapes, see:
- https://github.com/DnaCalc/OxVba#readme
- https://github.com/DnaCalc/OxVba/blob/master/docs/LANGUAGE_SERVICE_PUBLIC_INTERFACE.md
- https://github.com/DnaCalc/OxVba/blob/master/docs/spec/OXIDE_DIRECT_HOST_SESSION_FACADE_V1.md
- https://github.com/DnaCalc/OxVba/blob/master/docs/spec/COM_REFERENCE_SELECTION_SERVICE_V1.md
