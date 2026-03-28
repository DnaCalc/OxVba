# OxVBA

OxVBA is a VBA 7.x-compatible compiler/runtime in Rust with a broader hosting model than Office VBA itself.

The core idea is:
- keep the language/runtime engine close to VBA semantics
- make project layout, execution, and hosting practical outside Office
- use one code style and codebase across script-style, program-style, and library-style realizations

This README is the end-user guide for:
- source layouts in files and directories
- project formats
- startup behavior
- project options and references
- current CLI commands and runtime options

For repo process and implementation doctrine, see:
- `CHARTER.md`
- `OPERATIONS.md`
- `MACH1000_PLAN.md`
- `docs/AUTORUN_STATE.md` — minimal AutoRun control/sync file; current terminal gate target (`v620`)

## What OxVBA Supports

Current user-facing source/project lanes:
- run a single `.bas` file directly
- run a directory as a project by convention
- run an explicit `.basproj` project
- run a legacy `.vbp` project through the `VBP-S0` adapter
- build the same project targets through the same discovery rules
- compile a single source file into an `.oxb` bundle with the low-level `compile` command

Current canonical project format:
- `.basproj`

Legacy compatibility format:
- `.vbp` via a bounded adapter/import layer

Not supported yet:
- VB forms / startup forms
- designer-backed startup
- `Form=`, `UserControl=`, `PropertyPage=` `.vbp` surfaces

## Quick Start

Single-file script/program:

```powershell
oxvba run hello.bas
```

Run a directory by discovery:

```powershell
oxvba run-project .
```

Build a project target:

```powershell
oxvba build .
```

Import a legacy VB6 project file:

```powershell
oxvba import-vbp legacy\Project1.vbp
```

## Source Layouts

### 1. Single `.bas` File

The simplest lane is a single VBA module file:

```text
hello.bas
```

Example:

```vb
Option Explicit

Print "Hello from OxVBA"
```

Run it with:

```powershell
oxvba run hello.bas --profile windows-stdio
```

This is the most script-like lane. It is a good fit for:
- automation scripts
- console/stdIO tools
- experiments
- single-module utilities

### 2. Convention-Mode Directory

If a directory contains no `.basproj` and no `.vbp`, OxVBA treats it as a project by convention:

```text
my-tool/
  Main.bas
  Helpers.bas
  Calculator.cls
```

Run it with:

```powershell
oxvba run-project .\my-tool
```

Convention mode:
- recursively loads `.bas` and `.cls` files
- uses the directory name as the project name
- applies the normal startup ladder for executable runs

### 3. Explicit `.basproj` Project

This is the canonical OxVBA project format:

```text
finance-tools/
  FinanceTools.basproj
  Main.bas
  Pricing.bas
  Calculator.cls
```

Example:

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>FinanceTools</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
    <DefaultRuntimeProfile>windows-headless</DefaultRuntimeProfile>
    <DefaultPolicyPreset>deterministic-runtime</DefaultPolicyPreset>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Main.bas" />
    <Module Include="Pricing.bas" />
    <ClassModule Include="Calculator.cls">
      <VBExposed>True</VBExposed>
    </ClassModule>
  </ItemGroup>
</Project>
```

Run it with:

```powershell
oxvba run-project .\finance-tools
```

### 4. Legacy `.vbp` Project

OxVBA can run a bounded subset of VB6 `.vbp` files directly:

```text
legacy-app/
  Project1.vbp
  Main.bas
  Helpers.bas
```

Run it with:

```powershell
oxvba run-project .\legacy-app
```

Or explicitly:

```powershell
oxvba run-project .\legacy-app\Project1.vbp
```

If you want to convert it to the canonical format:

```powershell
oxvba import-vbp .\legacy-app\Project1.vbp
```

## Project Discovery Rules

For `oxvba run-project [PATH]` and `oxvba build [PATH]`, discovery is:

1. If `PATH` is a `.vbp` file, use the VBP adapter.
2. If `PATH` is a `.basproj` file, use the `.basproj` loader.
3. If `PATH` is a directory with a unique `.basproj`, use that project.
4. If `PATH` is a directory with no `.basproj` but a unique `.vbp`, use that project.
5. If `PATH` is a directory with neither, use convention mode.

Deterministic ambiguity rules:
- multiple `.basproj` files in one directory: error
- multiple `.vbp` files in one directory when no `.basproj` is present: error

`oxvba build` now follows the same discovery rules as `oxvba run-project`.

## Startup Semantics

For executable/program-style runs, the startup ladder is:

1. explicit configured entrypoint
2. else unique top-level mainline
3. else unique `Sub Main`
4. else deterministic error

Sources of an explicit entrypoint:
- `.basproj`: `<EntryPoint>Module.Procedure</EntryPoint>`
- `.vbp`: `Startup="Module.Procedure"`

Special case:
- `.vbp` `Startup="Sub Main"` means “use the fallback ladder”, not a literal invalid `EntryPoint`

### Top-Level Statements

OxVBA supports top-level executable statements for program-style/script-style execution.

Example:

```vb
Option Explicit

Dim x As Long
x = 41
Call Bump(x)
Print x

Public Sub Bump(ByRef value As Long)
    value = value + 1
End Sub
```

These top-level statements are lowered into an internal synthetic startup procedure.

Current rule:
- allowed for direct-file runs and `OutputType=Exe` program-style execution
- rejected for `Library`, `Addin`, `ComServer`, and `ComExe`

That rejection is intentional for now. It keeps those output forms conservative and leaves room to add a more permissive/tolerant policy later without breaking compatibility.

### What Is Not Supported Yet

Not part of the current startup model:
- startup forms
- designer-backed startup
- VB6 form lifecycle startup semantics

## `.basproj` Project Options

The main project properties are:

| Property | Meaning |
|----------|---------|
| `OutputType` | What the project produces |
| `ProjectName` | Logical project name |
| `EntryPoint` | Explicit startup procedure for executable runs |
| `RuntimeFlavor` | `Lite` or `Jit` |
| `DefaultRuntimeProfile` | Default host/runtime profile |
| `DefaultPolicyPreset` | Default host policy preset |
| `DefaultRootObject` | Default injected root object name |
| `DefineConstants` | Conditional compilation constants |

### `OutputType`

Supported values:

| OutputType | Meaning |
|-----------|---------|
| `HostModule` | host-loaded module/bundle lane |
| `Library` | library-style output |
| `Exe` | executable/program-style output |
| `Addin` | add-in style output |
| `ComServer` | in-process COM server |
| `ComExe` | out-of-process COM executable |

Practical rule today:
- `Exe` is the main program/script lane
- `Library`, `Addin`, `ComServer`, `ComExe` are non-mainline lanes and currently reject top-level executable statements

## Module Item Types

Main item types in `.basproj`:

| Item | Meaning |
|------|---------|
| `<Module Include="...">` | procedural `.bas` module |
| `<ClassModule Include="...">` | class `.cls` module |
| `<DocumentModule Include="...">` | host document/code-behind module |

Useful class metadata:
- `VBExposed`
- `VBPredeclaredId`
- `VBGlobalNamespace`
- `VBCreatable`

COM-oriented class metadata for COM server outputs:
- `Instancing`
- `ProgId`
- `Description`

## References

OxVBA project references are ordered. Reference order matters.

### `.basproj` References

Supported reference item types:

| Item | Meaning |
|------|---------|
| `<ProjectReference Include="...">` | reference another project |
| `<COMReference Include="...">` | reference a COM/type library |
| `<NativeReference Include="...">` | reference a native library used by `Declare` |

Example:

```xml
<ItemGroup>
  <ProjectReference Include="..\CoreLib\CoreLib.basproj" />
  <COMReference Include="Scripting">
    <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>
    <VersionMajor>1</VersionMajor>
    <VersionMinor>0</VersionMinor>
    <Lcid>0</Lcid>
    <ImportLib>scrrun.dll</ImportLib>
  </COMReference>
</ItemGroup>
```

### `.vbp` References

Current `VBP-S0` reference support:

| `.vbp` form | Meaning |
|-------------|---------|
| `Reference=*\G...` | ordered type-library/COM reference |
| `Reference=*\A...` | ordered project reference to `.vbp` / `.basproj` |

Not all historical VB6 reference surfaces are supported. Current `.vbp` support is intentionally narrow and deterministic.

## Native Exports

For library/add-in style projects, `.basproj` can declare native exports:

```xml
<ItemGroup>
  <NativeExport Include="CalcBlackScholes">
    <Module>PricingFunctions</Module>
    <Procedure>BlackScholes</Procedure>
    <CallingConvention>Stdcall</CallingConvention>
  </NativeExport>
</ItemGroup>
```

This is how OxVBA exposes selected public procedures as exported entrypoints for wrapper/native lanes.

## Current CLI Commands

The commands below describe the current implemented CLI, not the larger future command map.

### `oxvba run <file.bas>`

Runs a single `.bas` file directly.

Implemented options:
- `--jit`
- `--dump-slots`
- `--dump-values`
- `--dump-bootstrap`
- `--config <path>`
- `--profile <id>`
- `--policy <preset>`
- `--runtime-class <class>`
- `--allow-interaction <bool>`
- `--allow-process-spawn <bool>`
- `--allow-filesystem-mutation <bool>`
- `--allow-dynamic-link <bool>`
- `--allow-com-activation <bool>`
- `--deterministic-mode <bool>`
- `--ui-virtualization <mode>`
- `--unsupported-mode <mode>`
- `--wasm-runtime-class <class>`

Examples:

```powershell
oxvba run hello.bas
oxvba run hello.bas --profile windows-stdio
oxvba run tool.bas --jit --dump-values
oxvba run tool.bas --policy strict-ci --allow-dynamic-link false
```

### `oxvba run-project [PATH]`

Runs a discovered project target.

Implemented options:
- `--entry <Module.Procedure>`
- `--jit`
- `--dump-slots`
- `--dump-values`
- `--dump-bootstrap`
- `--profile <id>`
- `--policy <preset>`
- same runtime/bootstrap override flags as `oxvba run`:
  `--config`, `--runtime-class`, `--allow-interaction`,
  `--allow-process-spawn`, `--allow-filesystem-mutation`,
  `--allow-dynamic-link`, `--allow-com-activation`,
  `--deterministic-mode`, `--ui-virtualization`,
  `--unsupported-mode`, `--wasm-runtime-class`

Examples:

```powershell
oxvba run-project .
oxvba run-project .\app
oxvba run-project .\FinanceTools.basproj
oxvba run-project .\legacy\Project1.vbp
oxvba run-project .\app --entry Startup.Boot
oxvba run-project . --profile windows-stdio --jit
oxvba run-project . --runtime-class linux-stdio --allow-dynamic-link false
```

### `oxvba build [PATH]`

Builds a discovered project target into an `.oxb` bundle.

Implemented options:
- `-o <path>`
- `--output <path>`

Examples:

```powershell
oxvba build .
oxvba build .\FinanceTools.basproj
oxvba build .\legacy\Project1.vbp
oxvba build . -o .\dist\FinanceTools.oxb
```

Default output path:
- explicit file input: output beside the input target, named from `ProjectName`
- directory input: output inside that directory as `<ProjectName>.oxb`

### `oxvba compile <input>`

Compiles a single source file directly into an `.oxb` bundle without going through the project/discovery path.

Implemented options:
- `-o <path>`
- `--output <path>`

Example:

```powershell
oxvba compile .\legacy\Module1.bas
oxvba compile .\legacy\Module1.bas -o .\dist\Module1.oxb
```

### `oxvba init [PATH]`

Creates a minimal `.basproj` scaffold.

Implemented options:
- `--kind <application|library|addin|host-module|com-server|com-exe>`

Example:

```powershell
oxvba init .\new-app
oxvba init .\new-lib --kind library
oxvba init .\excel-host --kind host-module
oxvba init .\calc-com --kind com-server
```

### `oxvba import-vbp <file.vbp>`

Imports a supported legacy `.vbp` into `.basproj`.

Implemented options:
- `-o <path>`
- `--output <path>`

Example:

```powershell
oxvba import-vbp .\legacy\Project1.vbp
oxvba import-vbp .\legacy\Project1.vbp -o .\Project1.basproj
```

## Runtime Profiles and Policies

Two important concepts:

- runtime profile: what host/runtime environment OxVBA assumes
- policy preset / overrides: what that host is allowed to do

Examples of runtime classes currently recognized by the CLI:
- `windows-headless`
- `windows-stdio`
- `windows-gui`
- `linux-stdio`
- `linux-headless`
- `macos-gui`
- `macos-headless`

These are selected primarily via `--profile` or lower-level overrides.

For console/stdIO work, the main practical profiles are:
- `windows-stdio`
- `linux-stdio`

## Recommended Layouts

### Console/Tool App

```text
my-tool/
  MyTool.basproj
  Main.bas
  CommandLine.bas
  Formatting.bas
```

Use:
- `OutputType=Exe`
- top-level mainline or explicit `EntryPoint`
- `windows-stdio` / `linux-stdio` when appropriate

### Library

```text
my-lib/
  MyLib.basproj
  PublicApi.bas
  InternalHelpers.bas
  Widget.cls
```

Use:
- `OutputType=Library`
- no top-level mainline
- explicit exported/public surface

### Host-Embedded Module

```text
host-module/
  HostModule.basproj
  Module1.bas
  ThisWorkbook.cls
  Sheet1.cls
```

Use:
- `OutputType=HostModule`
- no startup mainline requirement
- host-injected root object and host-driven execution lifecycle

### Add-in

```text
my-addin/
  MyAddin.basproj
  Functions.bas
  Registration.bas
```

Use:
- `OutputType=Addin`
- no top-level mainline
- native exports / add-in metadata as needed

### COM Server

```text
my-com/
  MyCom.basproj
  ServerHelpers.bas
  Widget.cls
  Factory.cls
```

Use:
- `OutputType=ComServer` or `ComExe`
- no top-level mainline
- creatable/exposed classes with COM metadata

## Current Limits to Keep in Mind

- `.basproj` is the canonical format; `.vbp` is an adapter lane
- forms/designer-backed startup are not supported yet
- top-level executable statements are for program-style execution, not library/add-in/com-server outputs
- `.vbp` support is intentionally strict and deterministic

## Further Reading

- [BASPROJ_SPEC_V1.md](C:/Work/DnaCalc/OxVba/docs/spec/BASPROJ_SPEC_V1.md)
- [HOSTING_PROJECT_TOOLING_PROPOSAL.md](C:/Work/DnaCalc/OxVba/docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md)
- [VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md](C:/Work/DnaCalc/OxVba/docs/spec/VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md)

## Quick Verification

```powershell
./scripts/meta-check.ps1 -Fast
./scripts/run-smoke.ps1
```

Optional heavier lanes:

```powershell
./scripts/run-conformance.ps1
./scripts/run-matrix.ps1
```
