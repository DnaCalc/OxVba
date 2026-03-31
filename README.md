# OxVBA

OxVBA is a Rust implementation of the VBA 7.x language/runtime with a broader project and hosting model than the Office-bundled VBA engine.

In practical terms, OxVBA lets you:
- run a single `.bas` file directly
- run a directory as a project by convention
- define an explicit project in the canonical `.basproj` format
- import and run a bounded legacy `.vbp` subset through a deterministic adapter
- build compiled `.oxb` bundles from the same project layouts you run
- choose host/runtime profiles and policy presets instead of being locked to one Office host

This README is the primary user-facing overview for evaluating and starting to use OxVBA. It explains:
- what OxVBA is
- how to install and first-run it
- how source files, project files, and references work
- what parity you should expect today
- where OxVBA goes beyond VBA 7.1 in Excel
- what compilation and host/runtime options exist
- what cross-platform use currently means

For deeper specifications and validation truth, see:
- [docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md](docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md)
- [docs/spec/BASPROJ_SPEC_V1.md](docs/spec/BASPROJ_SPEC_V1.md)
- [docs/spec/VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md](docs/spec/VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md)
- [docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv](docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv)
- [docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv](docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv)
- [docs/validation/COM_VALIDATION_MATRIX_V1.csv](docs/validation/COM_VALIDATION_MATRIX_V1.csv)

## What OxVBA Is

OxVBA is not just a parser or transpiler. It is a workspace of crates that covers:
- parsing and syntax infrastructure
- semantic analysis and project/reference lowering
- bytecode generation
- VM execution
- JIT execution
- host/runtime abstraction and policy control
- Windows COM interop for the supported subset
- project loading for `.basproj`, convention-mode directories, and bounded `.vbp`

The core user model is:
- write VBA-style modules and classes
- choose the right project lane for your problem
- run or build them through one CLI
- keep semantics close to VBA where OxVBA claims support
- make hosting and execution more explicit, deterministic, and portable than Office VBA

## Installation

OxVBA can be used either from a prebuilt Windows release or by building from source.

### Option 1: Windows x64 release

Download the latest Windows x64 release zip from GitHub Releases, unzip it, and use:

```powershell
.\oxvba.exe run hello.bas
```

The release zip is intended to run with no Rust toolchain or other developer dependencies installed. It includes:
- `oxvba.exe`: the main CLI
- `oxvba-run.exe`: the `.oxb` bundle launcher

### Option 2: build from source without installing

### Prerequisites

- Rust stable via `rustup`
- Git
- PowerShell on Windows for repo scripts

#### Get the repo

```powershell
git clone https://github.com/DnaCalc/OxVba.git
cd OxVba
```

#### Use the CLI without installing it

```powershell
cargo run -p oxvba-cli -- run hello.bas
```

This is the safest way to evaluate the repo because it always uses the checked-out source.

### Option 3: install the CLI from the repo

```powershell
cargo install --path crates/oxvba-cli
```

After that, use:

```powershell
oxvba-cli run hello.bas
```

In the rest of this README, commands are shown as `oxvba ...` for readability. If you are running from source or a source-built install today, replace that with `oxvba-cli ...`.

If you have not installed the CLI at all, replace it with:

```powershell
cargo run -p oxvba-cli -- ...
```

## First Run

Create a minimal file:

```vb
Print "Hello from OxVBA"
```

Save it as `hello.bas`, then run:

```powershell
oxvba run hello.bas
```

If you want to make the runtime choice explicit:

```powershell
oxvba run hello.bas --profile windows-stdio
```

If you want to override the runtime class directly:

```powershell
oxvba run hello.bas --runtime-class windows-stdio
```

That is the smallest useful OxVBA workflow: one file, one command, one engine.

## Quick Tour

| Goal | Recommended lane | Command |
|---|---|---|
| Run one quick script/tool | single `.bas` file | `oxvba run tool.bas` |
| Run several modules/classes without project metadata | convention-mode directory | `oxvba run-project .\my-tool` |
| Define metadata, references, entrypoint, output type | `.basproj` | `oxvba run-project .\MyApp.basproj` |
| Bring forward a supported VB6 subset | bounded `.vbp` adapter | `oxvba run-project .\Legacy\Project1.vbp` |
| Convert a legacy `.vbp` to the canonical format | `.vbp` import | `oxvba import-vbp .\Legacy\Project1.vbp` |
| Build a bundle artifact | discovered project build | `oxvba build .` |

Practical rule:
- start new work with `.basproj`
- use `run file.bas` for very small utilities and experiments
- use convention mode while a project is still informal
- treat `.vbp` as a compatibility/import lane, not the long-term authoring format

## Example Use Cases

### 1. Simplest possible run: one module

`hello.bas`

```vb
Print "Hello from OxVBA"
```

Run:

```powershell
oxvba run hello.bas
```

Good fit:
- scripts
- experiments
- one-off automation
- simple console/stdIO tools

### 2. Slightly more advanced: a small executable directory

```text
math-tool/
  Main.bas
  MathHelpers.bas
```

`Main.bas`

```vb
Dim total As Long
total = MathHelpers.Add(20, 22)
Print total
```

`MathHelpers.bas`

```vb
Option Explicit

Public Function Add(ByVal x As Long, ByVal y As Long) As Long
    Add = x + y
End Function
```

Run the directory directly:

```powershell
oxvba run-project .\math-tool
```

Convention mode:
- loads `.bas` and `.cls` files from the directory
- uses the directory name as the project name
- applies the normal startup ladder for executable runs

Notes about the source text:
- `Option Explicit` is useful when you want undeclared-variable checking. OxVBA supports it, but you do not need it in every file.
- `Attribute VB_Name` is usually not needed when the filename already gives the intended module name. For `MathHelpers.bas`, the module name is already `MathHelpers`.

Add `Attribute VB_Name` when the logical module identity must be explicit in the file text itself, for example when preserving a legacy imported module name:

```vb
Attribute VB_Name = "LegacyPricing"
```

That is most relevant for import/export fidelity and cases where source text is being moved independently of the original filename.

### 3. Canonical project file: `.basproj`

```text
finance-tools/
  FinanceTools.basproj
  Main.bas
  Pricing.bas
  Calculator.cls
```

`FinanceTools.basproj`

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>FinanceTools</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
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

Run:

```powershell
oxvba run-project .\finance-tools
```

`DefaultRuntimeProfile` and `DefaultPolicyPreset` are optional. If they are omitted, `oxvba run-project` inherits the normal local runner defaults for the current platform. Add them only when you want durable project-level host behavior.

Build:

```powershell
oxvba build .\finance-tools
```

### 4. Project references and COM references

`App.basproj`

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>App</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Main.bas" />
    <ProjectReference Include="..\Core\Core.basproj" />
    <COMReference Include="Scripting">
      <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
      <ImportLib>scrrun.dll</ImportLib>
    </COMReference>
  </ItemGroup>
</Project>
```

`Main.bas`

```vb
Public Sub Main()
    Dim fs As Scripting.FileSystemObject
    Set fs = New Scripting.FileSystemObject
    Print VersionString()
    Print fs.GetBaseName("report.csv")
End Sub
```

This shows two important OxVBA ideas:
- project references are explicit and ordered
- imported typelibs are explicit project metadata, not hidden host state

Before you commit those references into `.basproj`, you can also inject them ad hoc for a quick run:

```powershell
oxvba run-project .\scratch-app --project-ref ..\Core\Core.basproj --com-ref Scripting=scrrun.dll
```

That convenience surface is intentionally bounded:
- CLI-injected references are good for experiments and one-off runs
- durable reference truth still belongs in `.basproj`
- `--com-ref` accepts either a bare library name or `Library=ImportLib` when you want to supply a stronger typelib hint

### 5. Legacy `.vbp` import

If you have a supported legacy VB6 project:

```powershell
oxvba import-vbp .\legacy\Project1.vbp
```

That generates a `.basproj` representation. You can then treat the imported project as the canonical project artifact moving forward.

## Source Layouts and Project Lanes

### Single `.bas` file

Use this for script-style execution:

```text
hello.bas
```

Run:

```powershell
oxvba run hello.bas
```

### Convention-mode directory

Use this when you want several modules but not a project file yet:

```text
my-tool/
  Main.bas
  Helpers.bas
  Widget.cls
```

Run:

```powershell
oxvba run-project .\my-tool
```

### `.basproj`

This is the canonical OxVBA project format.

Use it when you need:
- explicit output type
- explicit entrypoint
- references
- root-object defaults
- host/policy defaults
- stable project metadata

### `.vbp`

OxVBA supports a bounded legacy `.vbp` subset through the `VBP-S0` adapter/import lane.

Use it when:
- evaluating or running a legacy VB6-style project inside the supported subset
- importing old work into `.basproj`

Do not treat `.vbp` as the preferred authoring format for new OxVBA projects.

## Project Discovery Rules

For `oxvba run-project [PATH]` and `oxvba build [PATH]`, discovery is:

1. if `PATH` is a `.vbp` file, use the VBP adapter
2. if `PATH` is a `.basproj` file, use the `.basproj` loader
3. if `PATH` is a directory with a unique `.basproj`, use that project
4. if `PATH` is a directory with no `.basproj` but a unique `.vbp`, use that project
5. if `PATH` is a directory with neither, use convention mode

Deterministic ambiguity rules:
- multiple `.basproj` files in one directory: error
- multiple `.vbp` files in one directory when no `.basproj` is present: error

`oxvba build` follows the same discovery rules as `oxvba run-project`.

## Startup and Top-Level Execution

For executable/program-style runs, the startup ladder is:

1. explicit configured entrypoint
2. else unique top-level mainline
3. else unique `Sub Main`
4. else deterministic error

Sources of an explicit entrypoint:
- `.basproj`: `<EntryPoint>Module.Procedure</EntryPoint>`
- `.vbp`: `Startup="Module.Procedure"`

Special case:
- `.vbp` `Startup="Sub Main"` means "use the fallback ladder", not a literal invalid entrypoint

### Top-level statements

OxVBA supports top-level executable statements in program/script lanes.

Example:

```vb
Dim x As Long
x = 41
Call Bump(x)
Print x

Public Sub Bump(ByRef value As Long)
    value = value + 1
End Sub
```

Current rule:
- allowed for direct-file runs and `OutputType=Exe`
- rejected for `Library`, `Addin`, `ComServer`, and `ComExe`

That rejection is intentional. It keeps non-mainline outputs deterministic rather than silently ignoring executable top-level code.

### Startup/output matrix

| Output/lane | Top-level executable statements | Explicit entrypoint | `Sub Main` fallback | Notes |
|---|---|---|---|---|
| direct `run file.bas` | allowed | not applicable | not applicable | script/program lane |
| convention-mode directory | allowed | not applicable in directory metadata | yes | auto-loaded executable lane |
| `.basproj` with `OutputType=Exe` | allowed | yes | yes | canonical program/app lane |
| `.vbp` with `Type=Exe` in `VBP-S0` | allowed | yes | yes | bounded legacy adapter lane |
| `.basproj` with `OutputType=Library` | rejected | allowed for metadata completeness | no | non-mainline lane |
| `.basproj` with `OutputType=Addin` | rejected | optional | no | non-mainline lane |
| `.basproj` with `OutputType=ComServer` | rejected | optional | no | non-mainline lane |
| `.basproj` with `OutputType=ComExe` | rejected | optional | no | non-mainline lane |

## `.basproj` Project Model

### Main properties

| Property | Meaning |
|---|---|
| `OutputType` | what the project produces |
| `ProjectName` | logical project name |
| `EntryPoint` | explicit startup procedure for executable runs |
| `RuntimeFlavor` | runtime flavor selector |
| `DefaultRuntimeProfile` | optional project-level fallback host/runtime profile |
| `DefaultPolicyPreset` | optional project-level fallback host policy preset |
| `DefaultRootObject` | default injected root object name |
| `DefineConstants` | conditional compilation constants |

### Output types

| OutputType | Meaning |
|---|---|
| `HostModule` | host-loaded module/bundle lane |
| `Library` | library-style output |
| `Exe` | executable/program-style output |
| `Addin` | add-in style output |
| `ComServer` | in-process COM server |
| `ComExe` | out-of-process COM executable |

Practical rule today:
- `Exe` is the main program/script lane
- `Library`, `Addin`, `ComServer`, and `ComExe` are valid project shapes, but they are more conservative execution lanes and currently reject top-level executable statements

### What `OutputType=Exe` means today

`OutputType=Exe` means "this project is an executable/program-style OxVBA project." In practice that controls:
- startup resolution
- whether top-level executable statements are allowed
- the mainline execution rules used by `run-project` and `build`

It does not currently mean that `oxvba build` emits a native Windows PE `.exe`.

The stable build artifact today is an `.oxb` bundle. That bundle is executed by OxVBA tooling, typically through `oxvba-run <bundle.oxb>`.

### Planned separation: `OutputType` vs `BuildTarget`

OxVBA is moving toward a clearer split between:
- `OutputType`: the semantic shape of the project
- `BuildTarget`: the physical thing emitted by the build

Planned direction:
- `OutputType=Exe` means console/program-style startup semantics
- a future `OutputType=WinExe` would mean windowed executable semantics with no console expectation
- build packaging would be chosen separately through a `BuildTarget` concept such as `Bundle`, `WrapperExe`, `WrapperDll`, and later `NativeExe` / `NativeDll`

That keeps the current `.oxb` bundle story explicit while leaving room for future true native executable and DLL targets without overloading `OutputType`.

### Module item types

| Item | Meaning |
|---|---|
| `<Module Include="...">` | procedural `.bas` module |
| `<ClassModule Include="...">` | class `.cls` module |
| `<DocumentModule Include="...">` | host document/code-behind module |

Useful class metadata:
- `VBExposed`
- `VBPredeclaredId`
- `VBGlobalNamespace`
- `VBCreatable`

COM-oriented class metadata:
- `Instancing`
- `ProgId`
- `Description`

## References

OxVBA references are ordered. Reference order matters.

### `.basproj` references

| Item | Meaning |
|---|---|
| `<ProjectReference Include="...">` | reference another project |
| `<COMReference Include="...">` | reference a COM/type library |
| `<NativeReference Include="...">` | reference a native library used by `Declare` |

### `.vbp` references

Current `VBP-S0` reference support:

| `.vbp` form | Meaning |
|---|---|
| `Reference=*\G...` | ordered type-library/COM reference |
| `Reference=*\A...` | ordered project reference to `.vbp` / `.basproj` |

Current `.vbp` support is intentionally narrow and deterministic.

Unsupported `.vbp` reference/dependency surfaces include:
- forms and designer metadata
- broader historical VB6 project metadata outside the strict `VBP-S0` subset

## Language Features and Expected Parity

OxVBA aims at VBA 7.1 semantics, but it does not claim "everything everywhere" parity.

The honest user-facing expectation today is:

| Area | What to expect |
|---|---|
| Core language/runtime | broad VBA-style execution through one compiler/VM/JIT pipeline; exact support is bounded by the validation matrices rather than by blanket "full parity" language |
| Project startup | deterministic supported subset across direct-file runs, convention-mode directories, `.basproj`, and bounded `.vbp` execution |
| Windows COM | active and tested bounded early-bound and late-bound subsets on Windows |
| Imported component oddities | some historical Excel VBIDE import/export quirks are explicitly bounded to Excel behavior, not claimed as OxVBA parity targets |
| Language services | bounded internal service surface exists; OxVBA does not currently claim full LSP parity |
| Formalization | scaffolded and active, but not proof closure |
| Project storage | `.basproj` and bounded VBP adapter roundtrip are supported; full MS-OVBA parity is not currently claimed |

Important user-facing examples of current parity boundaries:
- Windows COM is active and tested; non-Windows external COM remains explicitly unsupported
- host-sensitive functions such as `Shell`, `Dir`, and `Environ` have a documented host-backed subset plus deterministic policy/fallback behavior
- imported member-attribute edge cases can differ from Excel because Excel itself drops some metadata during VBIDE import/export; OxVBA documents those cases rather than hiding them
- broader MS-OVBA storage parity is still blocked by source-extraction depth, so OxVBA does not overclaim it

For exact current truth, use:
- [docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv](docs/validation/LANGUAGE_VALIDATION_MATRIX_V1.csv)
- [docs/validation/COM_VALIDATION_MATRIX_V1.csv](docs/validation/COM_VALIDATION_MATRIX_V1.csv)
- [docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv](docs/validation/PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv)
- [docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv](docs/validation/LANGUAGE_SERVICES_AND_FORMALIZATION_MATRIX_V1.csv)

## How OxVBA Goes Beyond VBA 7.1 in Excel

OxVBA is deliberately broader than Excel VBA in several ways.

### Language/execution model

- direct execution of a single `.bas` file
- convention-mode directory execution without an Office host or VBA project container
- top-level executable statements in program/script lanes
- one engine shared across VM and JIT execution paths

### Project model

- canonical `.basproj` project format
- deterministic project discovery rules
- explicit project references, COM references, and native references
- import path from bounded legacy `.vbp` into the canonical project model

### Hosting model

- explicit runtime profiles
- explicit policy presets and capability overrides
- host-root injection and `HostModule` / `DocumentModule` project shapes
- deterministic unsupported-feature handling instead of silently leaning on one host's ambient behavior

### Runtime/build model

- build `.oxb` bundles from the same targets you run
- JIT is a first-class execution option
- native export metadata exists in the project model
- `Library`, `Addin`, `ComServer`, and `ComExe` are part of the project/output vocabulary

### Portability

- runtime-class selection is explicit instead of baked into one Office process model
- desktop runtime classes exist for Windows, Linux, and macOS
- WASI/browser/null-floor runtime classes are part of the host/runtime surface

That does not mean every extended surface is already feature-complete. It means OxVBA is intentionally designed as a broader execution/hosting platform than Excel VBA, with bounded and documented truth for each active lane.

## CLI, Compilation, and Host Options

### `oxvba run <file.bas>`

Runs a single module file directly.

Key options:
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
oxvba run values.bas --jit --dump-values
oxvba run hello.bas --dump-bootstrap
oxvba run empty.bas --policy strict-ci --allow-dynamic-link false
```

### `oxvba run-project [PATH]`

Runs a discovered project target.

Extra key option:
- `--entry <Module.Procedure>`
- `--project-ref <path>`
- `--com-ref <lib-or-lib=importlib>`
- `--native-ref <path>`

It also accepts the same runtime/bootstrap override flags as `run`.

Examples:

```powershell
oxvba run-project .
oxvba run-project .\FinanceTools.basproj
oxvba run-project .\legacy\Project1.vbp
oxvba run-project .\app --entry Startup.Boot
oxvba run-project . --profile windows-stdio --jit
oxvba run-project .\scratch-app --project-ref ..\Core\Core.basproj --com-ref Scripting=scrrun.dll
```

### `oxvba build [PATH]`

Builds a discovered project target into an `.oxb` bundle.

Options:
- `-o <path>`
- `--output <path>`
- `--project-ref <path>`
- `--com-ref <lib-or-lib=importlib>`
- `--native-ref <path>`

Examples:

```powershell
oxvba build .
oxvba build .\FinanceTools.basproj
oxvba build . -o .\dist\FinanceTools.oxb
oxvba build .\scratch-app --project-ref ..\Core\Core.basproj --com-ref Scripting=scrrun.dll
```

### `oxvba explain [PATH]` / `oxvba host-check [PATH]`

Shows the effective execution and reference configuration for a discovered project target.

It reports:
- discovered project lane
- startup choice
- output type
- effective runtime profile and policy preset
- the resolved bootstrap fingerprint
- effective reference order

Examples:

```powershell
oxvba explain .
oxvba explain .\FinanceTools.basproj --profile windows-stdio
oxvba host-check .\scratch-app --project-ref ..\Core\Core.basproj --com-ref Scripting=scrrun.dll
```

### Bundles and `.oxb`

An `.oxb` file is OxVBA's compiled bundle artifact. It packages the compiled program plus the metadata OxVBA needs to execute it deterministically.

Typical workflow:

```powershell
oxvba build .\demo-app -o .\dist\DemoApp.oxb
oxvba-run .\dist\DemoApp.oxb
oxvba-run .\dist\DemoApp.oxb --policy strict-ci
```

Important practical points:
- `.oxb` is the current stable compiled output format
- it is not a source archive; it is the compiled bundle you run
- running an `.oxb` bundle does not require Rust if you use the prebuilt Windows release binaries
- building OxVBA itself from source still requires Rust
- `oxvba-run` accepts the same runtime/bootstrap override flags as `oxvba run`, except there is no project-file default layer because it is launching an already-built bundle

### `oxvba compile <input>`

Compiles a single source file directly into an `.oxb` bundle without project discovery.

Examples:

```powershell
oxvba compile .\Module1.bas
oxvba compile .\Module1.bas -o .\dist\Module1.oxb
```

### `oxvba init [PATH]`

Creates a minimal `.basproj` scaffold.

Kinds:
- `application`
- `library`
- `addin`
- `host-module`
- `com-server`
- `com-exe`

Examples:

```powershell
oxvba init .\new-app
oxvba init .\new-lib --kind library
oxvba init .\excel-host --kind host-module
oxvba init .\calc-com --kind com-server
oxvba init .\legacy-tool --from-convention
```

`--from-convention` is the upgrade path from informal directory mode to an explicit project file. It scans the directory the same way `run-project` convention mode does, then writes a `.basproj` that captures the discovered modules and startup entrypoint.

### `oxvba import-vbp <file.vbp>`

Imports a supported legacy `.vbp` into `.basproj`.

Examples:

```powershell
oxvba import-vbp .\legacy\Project1.vbp
oxvba import-vbp .\legacy\Project1.vbp -o .\Project1.basproj
```

## Runtime profiles and policies

Two ideas matter a lot in OxVBA:
- runtime profile / runtime class: what host/runtime environment OxVBA should assume
- host policy: what that environment is allowed to do

Recognized runtime classes include:
- `host-native`
- `windows-stdio`
- `windows-gui`
- `windows-headless`
- `linux-stdio`
- `linux-headless`
- `macos-gui`
- `macos-headless`
- `wasi-local`
- `browser-sandbox`
- `null-floor`

Recognized host policy presets include:
- `strict-ci`
- `deterministic-runtime`
- `deterministic-compile-time`
- `interactive-dev`

UI virtualization modes:
- `disabled`
- `scripted-responses`
- `fail-on-prompt`

Unsupported-feature modes:
- `compile-time`
- `runtime`

Practical advice:
- local CLI runs already default to a practical local lane:
  - Windows: `windows-stdio + interactive-dev`
  - Linux: `linux-stdio + interactive-dev`
  - macOS: `macos-headless + interactive-dev`
- for CI or reproducible automation, start with `strict-ci` or an explicit deterministic preset
- set project defaults in `.basproj` only when you want durable execution behavior for that project
- `run-project` precedence is: CLI flags, then environment variables, then config file, then `.basproj` defaults, then built-in platform defaults
- scalar settings are overridden by the CLI; reference collections are additive unless the CLI supplies the same identity, in which case the CLI value replaces that item

## Cross-Platform Story

OxVBA is broader than Windows-only VBA, but the cross-platform story is layered.

### What is genuinely cross-platform today

- the language/compiler/runtime core is Rust-based and portable
- the project model is not tied to Office containers
- desktop runtime classes exist for Windows, Linux, and macOS
- host policies and runtime-class selection are explicit rather than hidden host defaults

### What is still Windows-specific

- live external COM interop
- Windows Office COM parity work
- imported Windows typelib behavior for the supported COM subset

Current architecture truth:
- Windows COM support is active and tested
- non-Windows external COM is explicitly unsupported

So the right expectation is:
- OxVBA as a language/runtime/project platform is cross-platform
- OxVBA as a Windows Office COM compatibility layer is currently strongest on Windows

## Current Limits

Keep these limits in mind when evaluating the project:

- `.basproj` is the canonical format; `.vbp` is an adapter/import lane
- forms and designer-backed startup are not supported
- non-mainline outputs reject top-level executable statements
- `.vbp` support is intentionally strict and deterministic
- non-Windows external COM is unsupported
- full MS-OVBA project-storage parity is not currently claimed
- language services are a bounded internal surface, not a full LSP claim
- formalization is active but not proof-complete

## Roadmap Notes

Two packaging directions are explicitly on the roadmap:
- wrapper targets built on top of compiled OxVBA artifacts, such as self-contained executable and DLL wrappers
- later true native-image targets for EXE and DLL outputs once wrapper parity and build semantics are stable

The intended long-term model is:
- `OutputType` chooses semantic startup/component shape
- `BuildTarget` chooses emitted artifact shape

## Recommended Starting Paths

If you are evaluating OxVBA, use one of these:

1. simplest script:

```powershell
oxvba run hello.bas
```

2. explicit application project:

```powershell
oxvba init .\demo-app
oxvba run-project .\demo-app
```

3. legacy project migration:

```powershell
oxvba import-vbp .\legacy\Project1.vbp
oxvba run-project .\legacy\Project1.basproj
```

4. bundle-oriented workflow:

```powershell
oxvba build .\demo-app -o .\dist\DemoApp.oxb
oxvba-run .\dist\DemoApp.oxb
```

## Verification

Fast repo checks:

```powershell
./scripts/meta-check.ps1 -Fast
./scripts/run-smoke.ps1
```

Fuller validation lanes:

```powershell
./scripts/run-conformance.ps1
./scripts/run-matrix.ps1
```

## Further Reading

- [docs/BUILDING.md](docs/BUILDING.md)
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- [docs/spec/BASPROJ_SPEC_V1.md](docs/spec/BASPROJ_SPEC_V1.md)
- [docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md](docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md)
- [docs/spec/VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md](docs/spec/VBP_SUBSET_AND_PROJECT_ARTIFACT_STRATEGY_DISCUSSION_V1.md)
