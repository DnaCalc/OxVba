# `.basproj` Project File Format Specification v1

Status: `normative-draft`
Date: 2026-03-23
Scope owner: OxVBA project system
Canonical path: `docs/spec/BASPROJ_SPEC_V1.md`
Supersedes: `oxvba.toml` format in `HOSTING_PROJECT_TOOLING_PROPOSAL.md` §4.1

Related docs:
- `docs/spec/HOSTING_PROJECT_TOOLING_PROPOSAL.md`
- `docs/spec/PROJECT_MODULE_REFERENCE_SPEC_V1.md`

---

## 1. Overview

The `.basproj` format is the canonical project file format for OxVBA projects. It uses MSBuild-compatible XML with SDK-style conventions:

- `<Project Sdk="...">` root element
- `<PropertyGroup>` for scalar properties
- `<ItemGroup>` for collections (modules, references, exports)
- `<Import>` for splitting content across files

The `Sdk` attribute (`OxVba.Sdk/0.1.0`) identifies the OxVBA SDK version and provides implicit defaults, analogous to `Microsoft.NET.Sdk` for .NET projects.

---

## 2. XML Schema

### 2.1 Root Element

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <!-- PropertyGroup and ItemGroup elements -->
</Project>
```

The `Sdk` attribute is required. Format: `OxVba.Sdk/<semver>`. The parser validates the SDK name prefix and extracts the version for compatibility checking.

### 2.2 PropertyGroup — Project Properties

All properties are optional unless noted. A project may contain multiple `<PropertyGroup>` elements; properties are merged in document order (last wins for duplicates).

| Property | Type | Values | Default | Required | Purpose |
|----------|------|--------|---------|----------|---------|
| `OutputType` | enum | `HostModule`, `Library`, `Exe`, `Addin`, `ComServer`, `ComExe` | — | **yes** | Semantic project/output kind |
| `ProjectName` | identifier | any valid VBA identifier | directory name | no | Maps to `ProjectManifest.project_name` |
| `EntryPoint` | string | `Module.Procedure` | — | no | Explicit startup procedure override for execution |
| `RuntimeFlavor` | enum | `Lite`, `Jit` | `Lite` | no | VM-only vs VM+JIT |
| `DefaultRuntimeProfile` | string | profile identifier | `windows-headless` | no | Default HAL runtime profile |
| `DefaultPolicyPreset` | string | preset identifier | `deterministic-runtime` | no | Default host policy preset |
| `DefaultRootObject` | string | identifier | `Application` | no | Host-injected root object name |
| `DefineConstants` | string | `KEY=VAL;KEY2=VAL2` | — | no | Conditional compilation constants |

**OutputType semantics:**

| OutputType | ProjectKind | Produces | Entry point |
|-----------|------------|---------|------------|
| `HostModule` | `Host` | `.oxb` bundle | not required |
| `Library` | `Library` | library-style project semantics | not required |
| `Exe` | `Source` | executable/program-style semantics | required via explicit `EntryPoint`, unique top-level mainline, or unique `Sub Main` |
| `Addin` | `Library` | add-in-style semantics | optional; top-level mainline rejected |
| `ComServer` | `Library` | in-process COM server semantics | not required (uses creatable classes) |
| `ComExe` | `Library` | out-of-process COM server semantics | not required (uses creatable classes) |

**OxVBA extension note:** top-level executable statements are an OxVBA hosting/project extension, not an Office-VBA parity claim. In `.basproj` program-style execution (`OutputType=Exe`), a module containing top-level executable statements may supply the startup mainline when no explicit `EntryPoint` is configured. In the current bounded lane, top-level executable statements are rejected for `Library`, `Addin`, `ComServer`, and `ComExe`.

**Packaging note:** `OutputType` is the semantic project kind, not a guarantee of today's emitted file shape. The current stable compiled output emitted by the shipped CLI is an OxVBA bundle artifact. Wrapper and native image packaging are a separate build-target concern.

**Planned extension:** a future `WinExe` `OutputType` is expected for windowed executable semantics distinct from console/program-style `Exe`. That future lane is intentionally separate from the physical build-target choice.

**DefineConstants format:** Semicolon-separated `KEY=VALUE` pairs. Values are parsed as `i32`. Keys without `=VALUE` default to `1`. Example: `VBA7=1;WIN64=1;DEBUG` → `{VBA7: 1, WIN64: 1, DEBUG: 1}`.

### 2.3 ItemGroup — Module Items

#### 2.3.1 `<Module>` — Procedural Modules

```xml
<Module Include="Module1.bas" />
```

Maps to `ModuleUnit` with `ModuleKind::Procedural`. The `Include` attribute is a relative path to the `.bas` source file. The module name defaults to the filename stem (without extension) unless an `Attribute VB_Name` line in the source overrides it.

#### 2.3.2 `<ClassModule>` — Class Modules

```xml
<ClassModule Include="Calculator.cls">
  <VBPredeclaredId>True</VBPredeclaredId>
  <VBExposed>True</VBExposed>
  <VBGlobalNamespace>False</VBGlobalNamespace>
  <VBCreatable>True</VBCreatable>
</ClassModule>
```

Maps to `ModuleUnit` with `ModuleKind::Class`. All metadata elements are optional booleans (default: `False`).

| Metadata | Maps to | Default |
|----------|---------|---------|
| `VBPredeclaredId` | `ModuleAttributes.vb_predeclared_id` | `False` |
| `VBExposed` | `ModuleAttributes.vb_exposed` | `False` |
| `VBGlobalNamespace` | `ModuleAttributes.vb_global_namespace` | `False` |
| `VBCreatable` | `ModuleAttributes.vb_creatable` | `False` |

#### 2.3.2.1 ClassModule COM Metadata

When a project uses `OutputType=ComServer` or `OutputType=ComExe`, `<ClassModule>` items may include additional metadata elements for COM registration and type library generation:

```xml
<ClassModule Include="Calculator.cls">
  <VBExposed>True</VBExposed>
  <VBCreatable>True</VBCreatable>
  <Instancing>MultiUse</Instancing>
  <ProgId>MyCOMLib.Calculator</ProgId>
  <Description>A basic calculator object</Description>
</ClassModule>
```

**`Instancing`** — Controls how COM clients create instances of the class. Enum values follow the VB6 instancing model:

| Value | Behavior |
|-------|----------|
| `Private` | Not visible outside the project. Cannot be created by external clients. |
| `PublicNotCreatable` | Visible to external clients via the type library, but can only be instantiated internally and passed out. |
| `MultiUse` | Externally creatable. Multiple clients share a single server process (relevant for `ComExe`). |
| `GlobalMultiUse` | Like `MultiUse`, but members are accessible without explicit instantiation (global namespace injection). |
| `SingleUse` | Externally creatable. Each `CreateObject` / `CoCreateInstance` call launches a new server process (`ComExe` only). |
| `GlobalSingleUse` | Like `SingleUse`, but members are accessible without explicit instantiation (`ComExe` only). |

Default: `Private` when `VBCreatable=False`; `MultiUse` when `VBCreatable=True`.

**`ProgId`** — The programmatic identifier used for `CreateObject("ProgId")` calls. Default value is `ProjectName.ClassName` (e.g., `MyCOMLib.Calculator`). Must be unique within the system registry.

**`Description`** — Freeform help text emitted into the IDL/TLB for the class. Appears in object browsers and tooling.

#### 2.3.3 `<DocumentModule>` — Code-Behind Modules

```xml
<DocumentModule Include="Sheet1.cls">
  <HostDocumentType>Worksheet</HostDocumentType>
</DocumentModule>
```

Maps to `ModuleUnit` with `ModuleKind::Document`. The `HostDocumentType` metadata is informational and stored in module attributes for host consumption.

### 2.4 ItemGroup — Reference Items

Reference declaration order (top-to-bottom, across `<ItemGroup>` elements) determines resolution precedence, matching the existing `ProjectReference.precedence_index` field.

#### 2.4.1 `<ProjectReference>` — Project References

```xml
<ProjectReference Include="..\CoreLib\CoreLib.basproj" />
```

Maps to `ProjectReference` with `ReferenceKind::Project`. The `Include` path is resolved relative to the directory containing the `.basproj` file.

#### 2.4.2 `<COMReference>` — COM Type Library References

```xml
<COMReference Include="Excel">
  <Guid>{00020813-0000-0000-C000-000000000046}</Guid>
  <VersionMajor>1</VersionMajor>
  <VersionMinor>9</VersionMinor>
  <Lcid>0</Lcid>
  <ImportLib>excel.exe</ImportLib>
</COMReference>
```

Maps to `TypeLibraryCatalogEntry`:

| XML Element | Internal Field |
|------------|---------------|
| `Include` attribute | `library_name` |
| `Guid` | `libid` (as `Option<String>`) |
| `VersionMajor` | `major_version` (u16) |
| `VersionMinor` | `minor_version` (u16) |
| `Lcid` | `lcid` (as `Option<u32>`) |
| `ImportLib` | `importlib` (primary resolution key) |

Cross-platform behavior: COMReference items produce `ReferenceBindingState::Failed` on non-Windows unless the host provides portable type library metadata blobs.

#### 2.4.3 `<NativeReference>` — Native Library References

```xml
<NativeReference Include="hostbridge">
  <Path>build/hostbridge.dll</Path>
</NativeReference>
```

Feeds `ExternalCallDescriptor.library` for `Declare ... Lib "name"` resolution.

### 2.5 ItemGroup — Native Export Items

#### 2.5.1 `<NativeExport>` — Exported Functions

```xml
<NativeExport Include="CalcBlackScholes">
  <Module>PricingFunctions</Module>
  <Procedure>BlackScholes</Procedure>
  <CallingConvention>Stdcall</CallingConvention>
  <Ordinal>1</Ordinal>
</NativeExport>
```

| Metadata | Type | Required | Default |
|----------|------|----------|---------|
| `Module` | string | **yes** | — |
| `Procedure` | string | **yes** | — |
| `CallingConvention` | enum | no | `Stdcall` |
| `Ordinal` | u16 | no | none |

**CallingConvention values:** `Stdcall`, `Cdecl`.

**Add-in metadata** (optional, used for XLL add-in registration):

| Metadata | Type | Purpose |
|----------|------|---------|
| `Category` | string | XLL function category displayed in the Function Wizard |
| `Description` | string | Function description text shown in the Function Wizard |
| `ArgumentDescriptions` | string | Pipe-delimited descriptions for each argument, in parameter order |

Example with add-in metadata:

```xml
<NativeExport Include="CalcBlackScholes">
  <Module>PricingFunctions</Module>
  <Procedure>BlackScholes</Procedure>
  <CallingConvention>Stdcall</CallingConvention>
  <Category>Financial</Category>
  <Description>Calculates the Black-Scholes option price</Description>
  <ArgumentDescriptions>Spot price|Strike price|Time to expiry (years)|Risk-free rate|Volatility</ArgumentDescriptions>
</NativeExport>
```

These metadata elements are ignored for non-Addin output types.

**Validation rules:**
1. Referenced `Module.Procedure` must exist and be `Public`
2. Must be in a `Procedural` module (not class)
3. Exported names (`Include` attribute) must be unique
4. For `OutputType=Library`: at least one export should exist (warning)
5. For `OutputType=Exe`/`HostModule`: exports are ignored with a warning

### 2.6 `<Import>` — File Inclusion

```xml
<Import Project="NativeExports.items" />
```

Standard MSBuild `<Import>` mechanism. The imported file uses the same `<Project>` root with `<ItemGroup>` children. Path is resolved relative to the importing file's directory. Imported items are merged as if they appeared inline at the import point.

Optional existence check:
```xml
<Import Project="NativeExports.items" Condition="Exists('NativeExports.items')" />
```

---

## 3. Mapping to ProjectManifest

### 3.1 Property Mapping

| .basproj | ProjectManifest field |
|----------|----------------------|
| `ProjectName` | `project_name` (fallback: directory name) |
| `OutputType=HostModule` | `project_kind = ProjectKind::Host` |
| `OutputType=Library\|Addin\|ComServer\|ComExe` | `project_kind = ProjectKind::Library` |
| `OutputType=Exe` | `project_kind = ProjectKind::Source` |
| `DefineConstants` | `conditional_constants: BTreeMap<String, i32>` |

### 3.2 Module Mapping

| .basproj Item | ModuleKind |
|--------------|-----------|
| `<Module>` | `Procedural` |
| `<ClassModule>` | `Class` |
| `<DocumentModule>` | `Document` |

### 3.3 Reference Mapping

| .basproj Item | ReferenceKind |
|--------------|--------------|
| `<ProjectReference>` | `Project` |
| `<COMReference>` | `TypeLibrary` |

Host-injected references are not declared in `.basproj` — they are added at runtime by the host.

---

## 4. Auto-Discovery Convention

When a `.basproj` contains no `<Module>`, `<ClassModule>`, or `<DocumentModule>` items:

1. All `**/*.bas` files in the project directory (recursive) are treated as `<Module>` items
2. All `**/*.cls` files in the project directory (recursive) are treated as `<ClassModule>` items
3. Module names are derived from filename stems
4. For `OutputType=Exe`, startup resolution is: explicit `EntryPoint` if configured, else unique top-level mainline, else unique `Sub Main`
5. For `Library`, `Addin`, `ComServer`, and `ComExe`, top-level executable statements are rejected in the current bounded lane
6. Ambiguous or missing startup resolution fails deterministically

When any explicit module item is present, auto-discovery is disabled entirely.

---

## 5. Complete Examples

### 5.1 Use Case A: Embedded in Rich Host (Excel-like)

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>HostModule</OutputType>
    <ProjectName>VBAProject</ProjectName>
    <DefaultRootObject>Application</DefaultRootObject>
    <DefineConstants>VBA7=1;WIN64=1</DefineConstants>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Module1.bas" />
    <ClassModule Include="Calculator.cls" />
    <DocumentModule Include="Sheet1.cls">
      <HostDocumentType>Worksheet</HostDocumentType>
    </DocumentModule>
    <DocumentModule Include="ThisWorkbook.cls">
      <HostDocumentType>Workbook</HostDocumentType>
    </DocumentModule>
  </ItemGroup>
  <ItemGroup>
    <COMReference Include="Excel">
      <Guid>{00020813-0000-0000-C000-000000000046}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>9</VersionMinor>
      <Lcid>0</Lcid>
      <ImportLib>excel.exe</ImportLib>
    </COMReference>
  </ItemGroup>
</Project>
```

### 5.2 Use Case B: C ABI DLL (XLL / Native Exports)

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <ProjectName>FinanceAddIn</ProjectName>
    <RuntimeFlavor>Jit</RuntimeFlavor>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="AddInSetup.bas" />
    <Module Include="PricingFunctions.bas" />
    <ClassModule Include="PricingEngine.cls">
      <VBExposed>True</VBExposed>
      <VBPredeclaredId>True</VBPredeclaredId>
    </ClassModule>
  </ItemGroup>
  <ItemGroup>
    <ProjectReference Include="..\CoreMath\CoreMath.basproj" />
    <COMReference Include="Scripting">
      <Guid>{420B2830-E718-11CF-893D-00A0C9054228}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
      <ImportLib>scrrun.dll</ImportLib>
    </COMReference>
  </ItemGroup>
  <ItemGroup>
    <NativeExport Include="CalcBlackScholes">
      <Module>PricingFunctions</Module>
      <Procedure>BlackScholes</Procedure>
      <CallingConvention>Stdcall</CallingConvention>
    </NativeExport>
    <NativeExport Include="xlAutoOpen">
      <Module>AddInSetup</Module>
      <Procedure>AutoOpen</Procedure>
      <CallingConvention>Stdcall</CallingConvention>
    </NativeExport>
  </ItemGroup>
</Project>
```

### 5.3 Use Case C: Standalone Executable

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ReportGenerator</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Main.bas" />
    <Module Include="FileProcessor.bas" />
    <ClassModule Include="Report.cls" />
  </ItemGroup>
  <ItemGroup>
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

### 5.4 Minimal Convention-Driven Project

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
  </PropertyGroup>
</Project>
```

Auto-discovers `**/*.bas` and `**/*.cls`. For `OutputType=Exe`, startup resolves by explicit `EntryPoint`, else unique top-level mainline, else unique `Sub Main`.

### 5.5 Separate Export File

`NativeExports.items`:
```xml
<Project>
  <ItemGroup>
    <NativeExport Include="CalcBlackScholes">
      <Module>PricingFunctions</Module>
      <Procedure>BlackScholes</Procedure>
      <CallingConvention>Stdcall</CallingConvention>
    </NativeExport>
  </ItemGroup>
</Project>
```

Referenced from `.basproj`:
```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
  </PropertyGroup>
  <Import Project="NativeExports.items" />
</Project>
```

### 5.6 Use Case F: In-Process COM Server

```xml
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>ComServer</OutputType>
    <ProjectName>MyCOMLib</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Utilities.bas" />
    <ClassModule Include="Calculator.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>MyCOMLib.Calculator</ProgId>
      <Description>A basic calculator object</Description>
    </ClassModule>
    <ClassModule Include="Formatter.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <Description>String formatting utilities</Description>
    </ClassModule>
  </ItemGroup>
</Project>
```

No `<NativeExport>` items are needed — the compiler generates `DllGetClassObject`, `DllCanUnloadNow`, `DllRegisterServer`, and `DllUnregisterServer` entry points automatically for `ComServer` projects. Class registration is driven by the `<ClassModule>` items with `VBCreatable=True`.

---

## 6. Schema Evolution

- The `Sdk` version attribute controls schema compatibility.
- Parsers MUST reject SDK versions with a major version they do not support.
- New optional properties/items may be added within a minor version.
- Removing or changing semantics of existing elements requires a major version bump.
