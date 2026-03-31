param(
    [Parameter(Mandatory = $true)]
    [string]$OxvbaExe,

    [Parameter(Mandatory = $true)]
    [string]$OxvbaRunExe
)

$ErrorActionPreference = "Stop"
$OxvbaExe = (Resolve-Path $OxvbaExe).Path
$OxvbaRunExe = (Resolve-Path $OxvbaRunExe).Path

function Invoke-Checked {
    param(
        [string]$Label,
        [string]$Exe,
        [string[]]$Arguments,
        [string[]]$MustContain = @()
    )

    Write-Host "==> $Label"
    $output = & $Exe @Arguments 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE`n$output"
    }
    foreach ($needle in $MustContain) {
        if (-not $output.Contains($needle)) {
            throw "$Label missing expected text '$needle'`n$output"
        }
    }
    return $output
}

function Write-Utf8File {
    param(
        [string]$Path,
        [string]$Content
    )

    $parent = Split-Path -Parent $Path
    if ($parent) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    Set-Content -Path $Path -Value $Content
}

$workspace = Join-Path $env:TEMP ("oxvba-readme-release-verify-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $workspace | Out-Null

try {
    $helloPath = Join-Path $workspace "hello.bas"
    Write-Utf8File $helloPath @'
Print "Hello from OxVBA"
'@

    $emptyPath = Join-Path $workspace "empty.bas"
    Write-Utf8File $emptyPath @'
Sub Main()
End Sub
'@

    $valuesPath = Join-Path $workspace "values.bas"
    Write-Utf8File $valuesPath @'
Dim valueOut As Long
valueOut = 41
'@

    Invoke-Checked "README hello run" $OxvbaExe @("run", $helloPath) @("Hello from OxVBA")
    Invoke-Checked "README hello explicit profile" $OxvbaExe @("run", $helloPath, "--profile", "windows-stdio") @("Hello from OxVBA")
    Invoke-Checked "README hello explicit runtime class" $OxvbaExe @("run", $helloPath, "--runtime-class", "windows-stdio") @("Hello from OxVBA")
    Invoke-Checked "README values jit dump" $OxvbaExe @("run", $valuesPath, "--jit", "--dump-values")
    Invoke-Checked "README hello dump bootstrap" $OxvbaExe @("run", $helloPath, "--dump-bootstrap") @("BOOTSTRAP:")
    Invoke-Checked "README strict policy example" $OxvbaExe @("run", $emptyPath, "--policy", "strict-ci", "--allow-dynamic-link", "false")

    $mathTool = Join-Path $workspace "math-tool"
    Write-Utf8File (Join-Path $mathTool "Main.bas") @'
Dim total As Long
total = MathHelpers.Add(20, 22)
Print total
'@
    Write-Utf8File (Join-Path $mathTool "MathHelpers.bas") @'
Option Explicit

Public Function Add(ByVal x As Long, ByVal y As Long) As Long
    Add = x + y
End Function
'@
    Invoke-Checked "README convention math-tool" $OxvbaExe @("run-project", $mathTool) @("42")

    $financeTools = Join-Path $workspace "finance-tools"
    Write-Utf8File (Join-Path $financeTools "FinanceTools.basproj") @'
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
'@
    Write-Utf8File (Join-Path $financeTools "Main.bas") @'
Public Sub Main()
    Print Pricing.PriceLabel()
End Sub
'@
    Write-Utf8File (Join-Path $financeTools "Pricing.bas") @'
Public Function PriceLabel() As String
    PriceLabel = "price-ok"
End Function
'@
    Write-Utf8File (Join-Path $financeTools "Calculator.cls") @'
Public Function Ping() As Long
    Ping = 42
End Function
'@
    Invoke-Checked "README basproj run-project" $OxvbaExe @("run-project", $financeTools) @("price-ok")
    Invoke-Checked "README basproj build" $OxvbaExe @("build", $financeTools)
    Invoke-Checked "README explain basproj" $OxvbaExe @("explain", $financeTools, "--profile", "windows-stdio") @("lane:", "runtime-profile: windows-stdio")

    $coreDir = Join-Path $workspace "Core"
    $appDir = Join-Path $workspace "App"
    $scratchAppDir = Join-Path $workspace "scratch-app"
    Write-Utf8File (Join-Path $coreDir "Core.basproj") @'
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <ProjectName>Core</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Version.bas" />
  </ItemGroup>
</Project>
'@
    Write-Utf8File (Join-Path $coreDir "Version.bas") @'
Public Function VersionString() As String
    VersionString = "core-ok"
End Function
'@
    Write-Utf8File (Join-Path $appDir "App.basproj") @'
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
'@
    Write-Utf8File (Join-Path $appDir "Main.bas") @'
Public Sub Main()
    Dim fs As Scripting.FileSystemObject
    Set fs = New Scripting.FileSystemObject
    Print VersionString()
    Print fs.GetBaseName("report.csv")
End Sub
'@
    Invoke-Checked "README references and COM example" $OxvbaExe @("run-project", $appDir) @("core-ok", "report")

    Write-Utf8File (Join-Path $scratchAppDir "Main.bas") @'
Public Sub Main()
    Dim fs As Scripting.FileSystemObject
    Set fs = New Scripting.FileSystemObject
    Print VersionString()
    Print fs.GetBaseName("report.csv")
End Sub
'@
    Invoke-Checked "README ad hoc project and COM refs" $OxvbaExe @(
        "run-project",
        $scratchAppDir,
        "--project-ref",
        (Join-Path $coreDir "Core.basproj"),
        "--com-ref",
        "Scripting=scrrun.dll"
    ) @("core-ok", "report")
    Invoke-Checked "README host-check with refs" $OxvbaExe @(
        "host-check",
        $scratchAppDir,
        "--project-ref",
        (Join-Path $coreDir "Core.basproj"),
        "--com-ref",
        "Scripting=scrrun.dll"
    ) @("lane:", "references:", "Scripting")

    $legacyDir = Join-Path $workspace "legacy"
    Write-Utf8File (Join-Path $legacyDir "Project1.vbp") @'
Type=Exe
Name="Project1"
Startup="Sub Main"
Module=Main; Main.bas
'@
    Write-Utf8File (Join-Path $legacyDir "Main.bas") @'
Public Sub Main()
    Print "legacy-ok"
End Sub
'@
    Invoke-Checked "README import-vbp" $OxvbaExe @("import-vbp", (Join-Path $legacyDir "Project1.vbp"))
    Invoke-Checked "README import-vbp explicit output" $OxvbaExe @("import-vbp", (Join-Path $legacyDir "Project1.vbp"), "-o", (Join-Path $workspace "Project1.basproj"))
    Invoke-Checked "README run-project vbp" $OxvbaExe @("run-project", (Join-Path $legacyDir "Project1.vbp")) @("legacy-ok")
    Invoke-Checked "README run-project imported basproj" $OxvbaExe @("run-project", (Join-Path $legacyDir "Project1.basproj")) @("legacy-ok")

    $myTool = Join-Path $workspace "my-tool"
    Write-Utf8File (Join-Path $myTool "Main.bas") @'
Public Sub Main()
    Print Helpers.Message()
End Sub
'@
    Write-Utf8File (Join-Path $myTool "Helpers.bas") @'
Public Function Message() As String
    Message = "my-tool-ok"
End Function
'@
    Write-Utf8File (Join-Path $myTool "Widget.cls") @'
Public Function Ping() As Long
    Ping = 1
End Function
'@
    Invoke-Checked "README run-project convention my-tool" $OxvbaExe @("run-project", $myTool) @("my-tool-ok")

    $appOverride = Join-Path $workspace "app"
    Write-Utf8File (Join-Path $appOverride "App.basproj") @'
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>AppOverride</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Main.bas" />
    <Module Include="Startup.bas" />
  </ItemGroup>
</Project>
'@
    Write-Utf8File (Join-Path $appOverride "Main.bas") @'
Public Sub Main()
    Err.Raise 5
End Sub
'@
    Write-Utf8File (Join-Path $appOverride "Startup.bas") @'
Public Sub Boot()
    Print "entry-override-ok"
End Sub
'@
    Invoke-Checked "README run-project entry override" $OxvbaExe @("run-project", $appOverride, "--entry", "Startup.Boot") @("entry-override-ok")
    Push-Location $financeTools
    try {
        Invoke-Checked "README build current directory" $OxvbaExe @("build", ".")
    }
    finally {
        Pop-Location
    }

    $demoApp = Join-Path $workspace "demo-app"
    Invoke-Checked "README init new app" $OxvbaExe @("init", $demoApp)
    Invoke-Checked "README init new lib" $OxvbaExe @("init", (Join-Path $workspace "new-lib"), "--kind", "library")
    Invoke-Checked "README init host module" $OxvbaExe @("init", (Join-Path $workspace "excel-host"), "--kind", "host-module")
    Invoke-Checked "README init COM server" $OxvbaExe @("init", (Join-Path $workspace "calc-com"), "--kind", "com-server")
    $legacyTool = Join-Path $workspace "legacy-tool"
    Write-Utf8File (Join-Path $legacyTool "Main.bas") @'
Public Sub Main()
    Print "legacy-tool-ok"
End Sub
'@
    Write-Utf8File (Join-Path $legacyTool "Helpers.bas") @'
Public Function Message() As String
    Message = "helper-ok"
End Function
'@
    Invoke-Checked "README init from convention" $OxvbaExe @("init", $legacyTool, "--from-convention") @("captured convention project")
    Invoke-Checked "README run init app" $OxvbaExe @("run-project", $demoApp)
    Invoke-Checked "README run-project explicit profile jit" $OxvbaExe @("run-project", $demoApp, "--profile", "windows-stdio", "--jit")

    $distDir = Join-Path $workspace "dist"
    New-Item -ItemType Directory -Force -Path $distDir | Out-Null
    $demoBundle = Join-Path $distDir "DemoApp.oxb"
    Invoke-Checked "README build demo bundle" $OxvbaExe @("build", $demoApp, "-o", $demoBundle)
    Invoke-Checked "README run built bundle" $OxvbaRunExe @($demoBundle)
    Invoke-Checked "README run built bundle strict policy" $OxvbaRunExe @($demoBundle, "--policy", "strict-ci")

    $module1 = Join-Path $workspace "Module1.bas"
    Write-Utf8File $module1 @'
Sub Main()
End Sub
'@
    Invoke-Checked "README compile single module" $OxvbaExe @("compile", $module1)
    Invoke-Checked "README compile single module custom output" $OxvbaExe @("compile", $module1, "-o", (Join-Path $distDir "Module1.oxb"))

    Write-Host "README release verification completed successfully."
}
finally {
    if (Test-Path $workspace) {
        Remove-Item -Recurse -Force $workspace
    }
}
