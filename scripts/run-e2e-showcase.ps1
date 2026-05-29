param(
    [string]$RunId = (Get-Date -Format 'yyyyMMddTHHmmss'),
    [string]$EvidenceRoot = 'docs/evidence/showcase'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$root = Join-Path $repo $EvidenceRoot
$runDir = Join-Path $root "e2e_external_interfaces_$RunId"
$srcDir = Join-Path $runDir 'sources'
$logDir = Join-Path $runDir 'logs'
$artifactDir = Join-Path $runDir 'artifacts'
New-Item -ItemType Directory -Force -Path $srcDir, $logDir, $artifactDir | Out-Null

$results = New-Object System.Collections.Generic.List[object]

function Add-Result {
    param(
        [string]$Name,
        [string]$Category,
        [string]$Status,
        [int]$ExitCode,
        [string]$StdoutPath,
        [string]$StderrPath,
        [string]$Notes,
        [hashtable]$Checks = @{}
    )
    $results.Add([pscustomobject]@{
        name = $Name
        category = $Category
        status = $Status
        exitCode = $ExitCode
        stdout = if ($StdoutPath) { Resolve-Path -LiteralPath $StdoutPath -ErrorAction SilentlyContinue | ForEach-Object Path } else { $null }
        stderr = if ($StderrPath) { Resolve-Path -LiteralPath $StderrPath -ErrorAction SilentlyContinue | ForEach-Object Path } else { $null }
        notes = $Notes
        checks = $Checks
    }) | Out-Null
}

function Invoke-Captured {
    param(
        [string]$Name,
        [string]$Category,
        [string]$Exe,
        [string[]]$ArgList,
        [string]$ExpectStdoutContains = '',
        [string]$ExpectStderrContains = '',
        [switch]$ExpectFailure,
        [scriptblock]$Before = $null,
        [scriptblock]$After = $null,
        [string]$Notes = ''
    )
    if ($Before) { & $Before }
    $safe = ($Name -replace '[^A-Za-z0-9_.-]', '_')
    $stdout = Join-Path $logDir "$safe.stdout.log"
    $stderr = Join-Path $logDir "$safe.stderr.log"
    Push-Location $repo
    try {
        & $Exe @ArgList > $stdout 2> $stderr
        $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
    } finally {
        Pop-Location
    }
    $outText = if (Test-Path $stdout) { [string](Get-Content -Raw -LiteralPath $stdout) } else { '' }
    $errText = if (Test-Path $stderr) { [string](Get-Content -Raw -LiteralPath $stderr) } else { '' }
    if ($null -eq $outText) { $outText = '' }
    if ($null -eq $errText) { $errText = '' }
    $checks = @{
        expectFailure = [bool]$ExpectFailure
        stdoutContains = $ExpectStdoutContains
        stderrContains = $ExpectStderrContains
        stdoutMatched = if ($ExpectStdoutContains) { $outText.Contains($ExpectStdoutContains) } else { $true }
        stderrMatched = if ($ExpectStderrContains) { $errText.Contains($ExpectStderrContains) } else { $true }
    }
    if ($After) {
        $afterResult = & $After
        if ($afterResult -is [hashtable]) { foreach ($k in $afterResult.Keys) { $checks[$k] = $afterResult[$k] } }
    }
    $exitOk = if ($ExpectFailure) { $exitCode -ne 0 } else { $exitCode -eq 0 }
    $matchOk = [bool]$checks.stdoutMatched -and [bool]$checks.stderrMatched
    $extraOk = $true
    foreach ($k in $checks.Keys) {
        if ($k.EndsWith('Ok') -and -not [bool]$checks[$k]) { $extraOk = $false }
    }
    $status = if ($exitOk -and $matchOk -and $extraOk) { 'pass' } else { 'fail' }
    Add-Result -Name $Name -Category $Category -Status $status -ExitCode $exitCode -StdoutPath $stdout -StderrPath $stderr -Notes $Notes -Checks $checks
}

function Write-Utf8NoBom([string]$Path, [string]$Text) {
    $dir = Split-Path -Parent $Path
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

# Build the real external command surfaces once. Subsequent commands use the produced EXEs directly.
Invoke-Captured -Name 'build_cli_and_launcher' -Category 'toolchain' -Exe 'cargo' -ArgList @('build','-p','oxvba-cli','-p','oxvba-launcher') -ExpectStderrContains 'Finished' -Notes 'Rust build verifies the command-line integration binaries used by every following pass.'
$cli = Join-Path $repo 'target/debug/oxvba-cli.exe'
$runner = Join-Path $repo 'target/debug/oxvba-run.exe'

# 1. New single file: compile, run source, run compiled artifact, inspect artifact.
$single = Join-Path $srcDir 'single_file/HelloSlots.bas'
$singleOxb = Join-Path $artifactDir 'HelloSlots.oxb'
Write-Utf8NoBom $single @'
Sub Main()
Dim answer
Dim label
answer = 40 + 2
label = "single-file-ok"
End Sub
'@
Invoke-Captured -Name 'single_file_compile_to_oxb' -Category 'single-file' -Exe $cli -ArgList @('compile',$single,'-o',$singleOxb) -ExpectStdoutContains 'compiled' -After { @{ artifactExistsOk = (Test-Path $singleOxb); artifactBytes = if (Test-Path $singleOxb) { (Get-Item $singleOxb).Length } else { 0 } } } -Notes 'Compiles a brand-new .bas source into an OxBundle artifact.'
Invoke-Captured -Name 'single_file_run_source_vm' -Category 'single-file' -Exe $cli -ArgList @('run',$single,'--dump-values','--runtime-class','windows-stdio') -ExpectStdoutContains 'VALUES:i32:42|string:"single-file-ok"' -Notes 'Runs the source path through the CLI/host/VM stack and checks the slot snapshot.'
Invoke-Captured -Name 'single_file_run_compiled_launcher' -Category 'single-file' -Exe $runner -ArgList @($singleOxb) -ExpectStderrContains 'Long' -Notes 'Runs the serialized .oxb bundle through oxvba-run and checks executable snapshot output emitted by the launcher.'

# 2. Two-module project: cross-module calls, VM target, disabled JIT boundary, bundle output.
$proj = Join-Path $srcDir 'two_module_project'
New-Item -ItemType Directory -Force -Path $proj | Out-Null
Write-Utf8NoBom (Join-Path $proj 'Main.bas') @'
Public Sub Main()
Dim total
Dim doubled
Dim summary
total = MathHelpers.Add(20, 22)
doubled = Scale(total)
summary = MathHelpers.FormatLabel(doubled)
End Sub

Public Function Scale(ByVal value)
Scale = value * 2
End Function
'@
Write-Utf8NoBom (Join-Path $proj 'MathHelpers.bas') @'
Public Function Add(ByVal lhs, ByVal rhs)
Add = lhs + rhs
End Function

Public Function FormatLabel(ByVal value)
FormatLabel = "two-module=" & value
End Function
'@
$basproj = Join-Path $proj 'ShowcaseTwoModule.basproj'
Write-Utf8NoBom $basproj @'
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>ShowcaseTwoModule</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Main.bas" />
    <Module Include="MathHelpers.bas" />
  </ItemGroup>
</Project>
'@
$projOxb = Join-Path $artifactDir 'ShowcaseTwoModule.oxb'
Invoke-Captured -Name 'two_module_project_run_vm' -Category 'project' -Exe $cli -ArgList @('run-project',$basproj,'--dump-values','--runtime-class','windows-stdio') -ExpectStdoutContains 'VALUES:i32:42|i32:84|string:"two-module=84"' -Notes 'Project loader resolves two modules and executes a cross-module qualified call plus an unqualified local call.'
Invoke-Captured -Name 'two_module_project_jit_disabled' -Category 'project' -Exe $cli -ArgList @('run-project',$basproj,'--jit','--dump-values','--runtime-class','windows-stdio') -ExpectFailure -ExpectStderrContains 'JIT execution is not implemented' -Notes 'The purged JIT backend reports an explicit disabled-backend diagnostic rather than falling back to the VM.'
Invoke-Captured -Name 'two_module_project_build_bundle' -Category 'project' -Exe $cli -ArgList @('build',$basproj,'-o',$projOxb) -ExpectStdoutContains 'built' -After { @{ artifactExistsOk = (Test-Path $projOxb); artifactBytes = if (Test-Path $projOxb) { (Get-Item $projOxb).Length } else { 0 } } } -Notes 'Build target serializes the project to an OxBundle artifact.'
Invoke-Captured -Name 'two_module_project_run_bundle' -Category 'project' -Exe $runner -ArgList @($projOxb) -Notes 'The project bundle is accepted by oxvba-run. Current project-bundle launcher output is intentionally quieter than single-file slot bundles.'

# 3. Editing errors and diagnostics.
$badSyntax = Join-Path $srcDir 'diagnostics/BrokenSyntax.bas'
Write-Utf8NoBom $badSyntax @'
Sub Main()
Dim x
x =
End Sub
'@
Invoke-Captured -Name 'diagnostic_broken_assignment' -Category 'diagnostics' -Exe $cli -ArgList @('compile',$badSyntax,'-o',(Join-Path $artifactDir 'BrokenSyntax.oxb')) -ExpectFailure -ExpectStderrContains 'compile failed' -Notes 'Intentional editor-style broken assignment validates surfaced compiler diagnostics and non-zero exit status.'
$badProj = Join-Path $srcDir 'diagnostics_project'
New-Item -ItemType Directory -Force -Path $badProj | Out-Null
Write-Utf8NoBom (Join-Path $badProj 'Main.bas') @'
Public Sub Main()
Dim total
total = MissingHelpers.Add(1, 2)
End Sub
'@
$badProjFile = Join-Path $badProj 'BrokenProject.basproj'
Write-Utf8NoBom $badProjFile @'
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>BrokenProject</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="Main.bas" />
  </ItemGroup>
</Project>
'@
Invoke-Captured -Name 'diagnostic_missing_module_reference' -Category 'diagnostics' -Exe $cli -ArgList @('run-project',$badProjFile,'--dump-values') -ExpectFailure -ExpectStderrContains 'compile-time diagnostic' -Notes 'Intentional bad cross-module edit validates compile diagnostic propagation through the project command.'

# 4. Access/ACE/Jet database via external COM Automation.
$accessProj = Join-Path $srcDir 'access_jet_project'
New-Item -ItemType Directory -Force -Path $accessProj | Out-Null
$dbPath = Join-Path $artifactDir 'ShowcaseJet.accdb'
$dbLiteral = $dbPath.Replace('\','\\')
$provider = 'Microsoft.ACE.OLEDB.12.0'
$connectionString = "Provider=$provider;Data Source=$dbPath"
$connectionLiteral = $connectionString.Replace('\','\\')
function Find-AdoTypelib([string]$FileName) {
    $roots = @($env:ProgramFiles, ${env:ProgramFiles(x86)}, $env:CommonProgramFiles, ${env:CommonProgramFiles(x86)}) | Where-Object { $_ }
    foreach ($root in $roots) {
        foreach ($candidate in @(
            (Join-Path $root "Common Files/System/ado/$FileName"),
            (Join-Path $root "System/ado/$FileName"),
            (Join-Path $root "ado/$FileName")
        )) {
            if (Test-Path -LiteralPath $candidate) { return (Resolve-Path -LiteralPath $candidate).Path }
        }
    }
    return $null
}
function Find-AcedaoTypelib {
    # The ACE DAO typelib (ACEDAO.DLL) ships with the Access Database Engine. Click-to-Run
    # Office nests it under `root\OfficeNN`; MSI installs use `OfficeNN` or the shared dir.
    $roots = @($env:ProgramFiles, ${env:ProgramFiles(x86)}, $env:CommonProgramFiles, ${env:CommonProgramFiles(x86)}) | Where-Object { $_ }
    $relatives = @()
    foreach ($ver in @('16', '15', '14')) {
        $relatives += "Microsoft Office/root/Office$ver/ACEDAO.DLL"
        $relatives += "Microsoft Office/Office$ver/ACEDAO.DLL"
    }
    foreach ($ver in @('OFFICE16', 'OFFICE15', 'OFFICE14')) {
        $relatives += "Microsoft Shared/$ver/ACEDAO.DLL"
    }
    foreach ($root in $roots) {
        foreach ($rel in $relatives) {
            $candidate = Join-Path $root $rel
            if (Test-Path -LiteralPath $candidate) { return (Resolve-Path -LiteralPath $candidate).Path }
        }
    }
    return $null
}
Write-Utf8NoBom (Join-Path $accessProj 'AccessJetMain.bas') @"
Public Sub Main()
Dim catalog
Dim cn
Dim createResult
Dim insertedAda
Dim insertedGrace
Dim rs
Dim fieldName
Dim fieldScore
Dim nameValue
Dim scoreValue
Set catalog = CreateObject("ADOX.Catalog")
createResult = DispatchInvoke(catalog, "Create", "$connectionLiteral")
Set cn = CreateObject("ADODB.Connection")
Call DispatchInvoke(cn, "Open", "$connectionLiteral")
Call DispatchInvoke(cn, "Execute", "CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)")
insertedAda = DispatchInvoke(cn, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)")
insertedGrace = DispatchInvoke(cn, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)")
rs = DispatchInvoke(cn, "Execute", "SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2")
fieldName = DispatchInvoke(rs, "Fields", "Name")
fieldScore = DispatchInvoke(rs, "Fields", "Score")
nameValue = DispatchInvoke(fieldName, "Value")
scoreValue = DispatchInvoke(fieldScore, "Value")
End Sub
"@
$accessBasproj = Join-Path $accessProj 'AccessJetShowcase.basproj'
Write-Utf8NoBom $accessBasproj @'
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>AccessJetShowcase</ProjectName>
    <EntryPoint>AccessJetMain.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="AccessJetMain.bas" />
    <COMReference Include="ADODB">
      <ImportLib>msado15.dll</ImportLib>
    </COMReference>
    <COMReference Include="ADOX">
      <ImportLib>msadox.dll</ImportLib>
    </COMReference>
  </ItemGroup>
</Project>
'@
$comPreflightOk = $true
try {
    $null = New-Object -ComObject 'ADOX.Catalog'
    $null = New-Object -ComObject 'ADODB.Connection'
} catch {
    $comPreflightOk = $false
    Add-Result -Name 'access_jet_com_preflight' -Category 'access-jet-com' -Status 'blocked' -ExitCode 1 -StdoutPath $null -StderrPath $null -Notes "ADOX/ADODB COM preflight failed: $($_.Exception.Message)"
}
if ($comPreflightOk) {
    if (Test-Path $dbPath) { Remove-Item -Force $dbPath }
    Invoke-Captured -Name 'access_jet_create_insert_query' -Category 'access-jet-com' -Exe $cli -ArgList @('run-project',$accessBasproj,'--dump-values','--runtime-class','windows-stdio','--allow-com-activation','true','--allow-filesystem-mutation','true') -ExpectStdoutContains 'string:"Grace"|i32:99' -After { @{ databaseExistsOk = (Test-Path $dbPath); databaseBytes = if (Test-Path $dbPath) { (Get-Item $dbPath).Length } else { 0 } } } -Notes 'OxVba project carries COMReference metadata for ADO/ADOX and uses real Windows COM Automation to create an Access/ACE .accdb, insert two rows, select Grace back, and surface scalar field values in the VM snapshot.'

    $adodbTypelib = Find-AdoTypelib 'msado15.dll'
    $adoxTypelib = Find-AdoTypelib 'msadox.dll'
    if (-not $adodbTypelib -or -not $adoxTypelib) {
        Add-Result -Name 'access_jet_mixed_imported_com_create_insert_query' -Category 'access-jet-com' -Status 'blocked' -ExitCode 1 -StdoutPath $null -StderrPath $null -Notes 'ADO/ADOX typelib files were not found, so the mixed imported-COM Access/JET pass could not be run.'
    } else {
        $earlyProj = Join-Path $srcDir 'access_jet_mixed_imported_com_project'
        New-Item -ItemType Directory -Force -Path $earlyProj | Out-Null
        $earlyDbPath = Join-Path $artifactDir 'ShowcaseJetMixedImported.accdb'
        $earlyConnectionLiteral = "Provider=$provider;Data Source=$earlyDbPath".Replace('\','\\')
        $adodbImportLiteral = $adodbTypelib.Replace('\','\\')
        $adoxImportLiteral = $adoxTypelib.Replace('\','\\')
        Write-Utf8NoBom (Join-Path $earlyProj 'AccessJetMixedMain.bas') @"
Public Sub Main()
Dim catalog As New ADOX.Catalog
Dim cn As New ADODB.Connection
Dim rs
Dim fieldName
Dim fieldScore
Dim nameValue
Dim scoreValue
Call DispatchInvoke(catalog, "Create", "$earlyConnectionLiteral")
Call cn.Open("$earlyConnectionLiteral", "", "", 0)
Call cn.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 0, 0)
rs = cn.Execute("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 0, 0)
fieldName = DispatchInvoke(rs, "Fields", "Name")
fieldScore = DispatchInvoke(rs, "Fields", "Score")
nameValue = DispatchInvoke(fieldName, "Value")
scoreValue = DispatchInvoke(fieldScore, "Value")
End Sub
"@
        $earlyBasproj = Join-Path $earlyProj 'AccessJetMixedImportedShowcase.basproj'
        Write-Utf8NoBom $earlyBasproj @"
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>AccessJetMixedImportedShowcase</ProjectName>
    <EntryPoint>AccessJetMixedMain.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="AccessJetMixedMain.bas" />
    <COMReference Include="ADODB">
      <ImportLib>$adodbImportLiteral</ImportLib>
    </COMReference>
    <COMReference Include="ADOX">
      <ImportLib>$adoxImportLiteral</ImportLib>
    </COMReference>
  </ItemGroup>
</Project>
"@
        if (Test-Path $earlyDbPath) { Remove-Item -Force $earlyDbPath }
        Invoke-Captured -Name 'access_jet_mixed_imported_com_create_insert_query' -Category 'access-jet-com' -Exe $cli -ArgList @('run-project',$earlyBasproj,'--dump-values','--runtime-class','windows-stdio','--allow-com-activation','true','--allow-filesystem-mutation','true') -ExpectStdoutContains 'string:"Grace"|i32:99' -After { @{ databaseExistsOk = (Test-Path $earlyDbPath); databaseBytes = if (Test-Path $earlyDbPath) { (Get-Item $earlyDbPath).Length } else { 0 } } } -Notes 'Mixed imported-COM ADO/ADOX project imports real typelib files, declares ADOX.Catalog and ADODB.Connection with As New, invokes ADODB Open/Execute through metadata-backed calls, still uses DispatchInvoke for unsupported Catalog/Recordset/Field pieces, creates an Access/ACE .accdb, inserts rows, and queries Grace / 99 back.'


        $strictProj = Join-Path $srcDir 'access_jet_strict_early_bound_project'
        New-Item -ItemType Directory -Force -Path $strictProj | Out-Null
        $strictDbPath = Join-Path $artifactDir 'ShowcaseJetStrictEarlyBound.accdb'
        $strictConnectionLiteral = "Provider=$provider;Data Source=$strictDbPath".Replace('\','\')
        Write-Utf8NoBom (Join-Path $strictProj 'AccessJetStrictEarlyMain.bas') @"
Public Sub Main()
Dim catalog As New ADOX.Catalog
Dim cn As New ADODB.Connection
Dim rs As ADODB.Recordset
Dim fieldName As ADODB.Field
Dim fieldScore As ADODB.Field
Dim nameValue
Dim scoreValue
Dim bangNameValue
Dim bangScoreValue
Call catalog.Create("$strictConnectionLiteral")
Call cn.Open("$strictConnectionLiteral", "", "", 0)
Call cn.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 0, 0)
Call cn.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 0, 0)
Set rs = cn.Execute("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 0, 0)
Set fieldName = rs.Fields("Name")
Set fieldScore = rs.Fields("Score")
nameValue = fieldName.Value
scoreValue = fieldScore.Value
bangNameValue = rs!Name
bangScoreValue = rs!Score
End Sub
"@
        $strictBasproj = Join-Path $strictProj 'AccessJetStrictEarlyBoundShowcase.basproj'
        Write-Utf8NoBom $strictBasproj @"
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>AccessJetStrictEarlyBoundShowcase</ProjectName>
    <EntryPoint>AccessJetStrictEarlyMain.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="AccessJetStrictEarlyMain.bas" />
    <COMReference Include="ADODB">
      <ImportLib>$adodbImportLiteral</ImportLib>
    </COMReference>
    <COMReference Include="ADOX">
      <ImportLib>$adoxImportLiteral</ImportLib>
    </COMReference>
  </ItemGroup>
</Project>
"@
        if (Test-Path $strictDbPath) { Remove-Item -Force $strictDbPath }
        Invoke-Captured -Name 'access_jet_strict_early_bound_create_insert_query' -Category 'access-jet-com' -Exe $cli -ArgList @('run-project',$strictBasproj,'--dump-values','--runtime-class','windows-stdio','--allow-com-activation','true','--allow-filesystem-mutation','true') -ExpectStdoutContains 'string:"Grace"|i32:99|string:"Grace"|i32:99' -After { @{ databaseExistsOk = (Test-Path $strictDbPath); databaseBytes = if (Test-Path $strictDbPath) { (Get-Item $strictDbPath).Length } else { 0 } } } -Notes 'Strict natural-source Access/JET COM pass: no DispatchInvoke appears in the VBA source; ADOX.Catalog, ADODB.Connection, ADODB.Recordset, and ADODB.Field are imported types, Fields("Name").Value and rs!Name/rs!Score are exercised, and the compiler lowers the natural source into the current COM bridge internally.'
    }
}

# 4b. Access/ACE/Jet database via the DAO object model (Microsoft Access Database Engine).
# DAO objects are dispinterfaces obtained from method calls (not coclasses), and the DAO
# DBEngine CLSID carries no TypeLib registry association, so this exercises interface-scoped
# binding plus the get-or-call dispatch path that strict Jet requires. Mirrors the ADO trio:
# late-bound, mixed (typed methods + DispatchInvoke field access), and strict early binding.
$daoPreflightOk = $true
try {
    $null = New-Object -ComObject 'DAO.DBEngine.120'
} catch {
    $daoPreflightOk = $false
    Add-Result -Name 'access_jet_dao_com_preflight' -Category 'access-jet-dao' -Status 'blocked' -ExitCode 1 -StdoutPath $null -StderrPath $null -Notes "DAO.DBEngine.120 COM preflight failed: $($_.Exception.Message)"
}
$acedaoTypelib = Find-AcedaoTypelib
if ($daoPreflightOk -and -not $acedaoTypelib) {
    Add-Result -Name 'access_jet_dao_typelib_probe' -Category 'access-jet-dao' -Status 'blocked' -ExitCode 1 -StdoutPath $null -StderrPath $null -Notes 'ACEDAO.DLL typelib was not found, so the early-bound DAO passes could not be run.'
}
if ($daoPreflightOk -and $acedaoTypelib) {
    function Write-DaoProject {
        param([string]$ProjectDir, [string]$ProjectName, [string]$Source)
        New-Item -ItemType Directory -Force -Path $ProjectDir | Out-Null
        Write-Utf8NoBom (Join-Path $ProjectDir 'AccessJetDaoMain.bas') $Source
        $basproj = Join-Path $ProjectDir "$ProjectName.basproj"
        Write-Utf8NoBom $basproj @"
<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>$ProjectName</ProjectName>
    <EntryPoint>AccessJetDaoMain.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include="AccessJetDaoMain.bas" />
    <COMReference Include="DAO">
      <ImportLib>$acedaoTypelib</ImportLib>
    </COMReference>
  </ItemGroup>
</Project>
"@
        return $basproj
    }

    # Late-bound DAO: untyped variables, CreateObject, DispatchInvoke by member name.
    $daoLateDir = Join-Path $srcDir 'access_jet_dao_late_bound_project'
    $daoLateDb = Join-Path $artifactDir 'ShowcaseJetDaoLate.accdb'
    $daoLateBasproj = Write-DaoProject -ProjectDir $daoLateDir -ProjectName 'AccessJetDaoLateBound' -Source @"
Public Sub Main()
Dim engine
Dim db
Dim rs
Dim nameValue
Dim scoreValue
Set engine = CreateObject("DAO.DBEngine.120")
Set db = DispatchInvoke(engine, "CreateDatabase", "$daoLateDb", ";LANGID=0x0409;CP=1252;COUNTRY=0", 128)
Call DispatchInvoke(db, "Execute", "CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)")
Call DispatchInvoke(db, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)")
Call DispatchInvoke(db, "Execute", "INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)")
Set rs = DispatchInvoke(db, "OpenRecordset", "SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2")
nameValue = DispatchInvoke(DispatchInvoke(rs, "Fields", "Name"), "Value")
scoreValue = DispatchInvoke(DispatchInvoke(rs, "Fields", "Score"), "Value")
End Sub
"@
    if (Test-Path $daoLateDb) { Remove-Item -Force $daoLateDb }
    Invoke-Captured -Name 'access_jet_dao_late_bound_create_insert_query' -Category 'access-jet-dao' -Exe $cli -ArgList @('run-project', $daoLateBasproj, '--dump-values', '--runtime-class', 'windows-stdio', '--allow-com-activation', 'true', '--allow-filesystem-mutation', 'true') -ExpectStdoutContains 'string:"Grace"|i32:99' -After { @{ databaseExistsOk = (Test-Path $daoLateDb); databaseBytes = if (Test-Path $daoLateDb) { (Get-Item $daoLateDb).Length } else { 0 } } } -Notes 'Late-bound DAO: CreateObject("DAO.DBEngine.120") plus DispatchInvoke create the Access/ACE .accdb via the DAO object model, insert two rows, open a recordset, and read Grace / 99 back through dynamic name dispatch.'

    # Mixed DAO: typed declarations early-bind the DBEngine/Database methods; recordset field
    # access stays on DispatchInvoke.
    $daoMixedDir = Join-Path $srcDir 'access_jet_dao_mixed_project'
    $daoMixedDb = Join-Path $artifactDir 'ShowcaseJetDaoMixed.accdb'
    $daoMixedBasproj = Write-DaoProject -ProjectDir $daoMixedDir -ProjectName 'AccessJetDaoMixed' -Source @"
Public Sub Main()
Dim engine As DAO.DBEngine
Dim db As DAO.Database
Dim rs As DAO.Recordset
Dim nameValue
Dim scoreValue
Set engine = CreateObject("DAO.DBEngine.120")
Set db = engine.CreateDatabase("$daoMixedDb", ";LANGID=0x0409;CP=1252;COUNTRY=0", 128)
Call db.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 128)
Call db.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 128)
Call db.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 128)
Set rs = db.OpenRecordset("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 4, 0, 4)
nameValue = DispatchInvoke(DispatchInvoke(rs, "Fields", "Name"), "Value")
scoreValue = DispatchInvoke(DispatchInvoke(rs, "Fields", "Score"), "Value")
End Sub
"@
    if (Test-Path $daoMixedDb) { Remove-Item -Force $daoMixedDb }
    Invoke-Captured -Name 'access_jet_dao_mixed_create_insert_query' -Category 'access-jet-dao' -Exe $cli -ArgList @('run-project', $daoMixedBasproj, '--dump-values', '--runtime-class', 'windows-stdio', '--allow-com-activation', 'true', '--allow-filesystem-mutation', 'true') -ExpectStdoutContains 'string:"Grace"|i32:99' -After { @{ databaseExistsOk = (Test-Path $daoMixedDb); databaseBytes = if (Test-Path $daoMixedDb) { (Get-Item $daoMixedDb).Length } else { 0 } } } -Notes 'Mixed DAO: typed Dim ... As DAO.DBEngine/Database/Recordset early-bind CreateDatabase/Execute/OpenRecordset against the scoped DAO interfaces, while the recordset Fields("...").Value chain stays on DispatchInvoke.'

    # Strict early-bound DAO: no DispatchInvoke in the source. DAO.DBEngine/Database/Recordset/Field
    # are imported types; rs.Fields("Name").Value and rs!Name/rs!Score are exercised.
    $daoStrictDir = Join-Path $srcDir 'access_jet_dao_strict_early_bound_project'
    $daoStrictDb = Join-Path $artifactDir 'ShowcaseJetDaoStrictEarlyBound.accdb'
    $daoStrictBasproj = Write-DaoProject -ProjectDir $daoStrictDir -ProjectName 'AccessJetDaoStrictEarlyBound' -Source @"
Public Sub Main()
Dim engine As DAO.DBEngine
Dim db As DAO.Database
Dim rs As DAO.Recordset
Dim fieldName As DAO.Field
Dim fieldScore As DAO.Field
Dim nameValue
Dim scoreValue
Dim bangNameValue
Dim bangScoreValue
Set engine = CreateObject("DAO.DBEngine.120")
Set db = engine.CreateDatabase("$daoStrictDb", ";LANGID=0x0409;CP=1252;COUNTRY=0", 128)
Call db.Execute("CREATE TABLE ShowcaseRecords (Id INTEGER, Name TEXT(50), Score INTEGER)", 128)
Call db.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (1, 'Ada', 98)", 128)
Call db.Execute("INSERT INTO ShowcaseRecords (Id, Name, Score) VALUES (2, 'Grace', 99)", 128)
Set rs = db.OpenRecordset("SELECT Name, Score FROM ShowcaseRecords WHERE Id = 2", 4, 0, 4)
Set fieldName = rs.Fields("Name")
Set fieldScore = rs.Fields("Score")
nameValue = fieldName.Value
scoreValue = fieldScore.Value
bangNameValue = rs!Name
bangScoreValue = rs!Score
End Sub
"@
    if (Test-Path $daoStrictDb) { Remove-Item -Force $daoStrictDb }
    Invoke-Captured -Name 'access_jet_dao_strict_early_bound_create_insert_query' -Category 'access-jet-dao' -Exe $cli -ArgList @('run-project', $daoStrictBasproj, '--dump-values', '--runtime-class', 'windows-stdio', '--allow-com-activation', 'true', '--allow-filesystem-mutation', 'true') -ExpectStdoutContains 'string:"Grace"|i32:99|string:"Grace"|i32:99' -After { @{ databaseExistsOk = (Test-Path $daoStrictDb); databaseBytes = if (Test-Path $daoStrictDb) { (Get-Item $daoStrictDb).Length } else { 0 } } } -Notes 'Strict natural-source DAO pass: no DispatchInvoke in the VBA source; DAO.DBEngine, DAO.Database, DAO.Recordset, and DAO.Field are imported types, Fields("Name").Value and rs!Name/rs!Score are exercised, and the compiler lowers the natural source into the COM bridge internally.'
}

# 5. Immediate window / REPL transcript.
$immInput = Join-Path $artifactDir 'immediate_input.txt'
Write-Utf8NoBom $immInput @'
.module
? MathHelpers.Add(5, 7)
? Scale(9)
.module MathHelpers
? Add(100, 23)
reset
.quit
'@
$safe = 'immediate_window_transcript'
$immOut = Join-Path $logDir "$safe.stdout.log"
$immErr = Join-Path $logDir "$safe.stderr.log"
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $cli
$psi.WorkingDirectory = $repo
foreach ($a in @('immediate',$basproj,'--module','Main','--runtime-class','windows-stdio')) { [void]$psi.ArgumentList.Add($a) }
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($psi)
$p.StandardInput.Write((Get-Content -Raw -LiteralPath $immInput))
$p.StandardInput.Close()
$out = $p.StandardOutput.ReadToEnd()
$err = $p.StandardError.ReadToEnd()
$p.WaitForExit()
Write-Utf8NoBom $immOut $out
Write-Utf8NoBom $immErr $err
Add-Result -Name 'immediate_window_transcript' -Category 'immediate' -Status ($(if ($p.ExitCode -eq 0 -and $out.Contains('12') -and $out.Contains('18') -and $out.Contains('123') -and $out.Contains('reset')) { 'pass' } else { 'fail' })) -ExitCode $p.ExitCode -StdoutPath $immOut -StderrPath $immErr -Notes 'Exercises the bounded Immediate Window interface over the two-module project: current module query, value evaluation, module retargeting, reset, and quit.' -Checks @{ transcriptMatchedOk = ($out.Contains('12') -and $out.Contains('18') -and $out.Contains('123') -and $out.Contains('reset')) }

$summary = [pscustomobject]@{
    runId = $RunId
    generatedUtc = (Get-Date).ToUniversalTime().ToString('o')
    repo = $repo
    runDir = $runDir
    results = $results
}
$summaryPath = Join-Path $runDir 'summary.json'
$summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $summaryPath -Encoding utf8

function HtmlEncode([string]$s) { [System.Net.WebUtility]::HtmlEncode($s) }
function Read-Short([string]$p, [int]$max = 1200) {
    if (-not $p -or -not (Test-Path $p)) { return '' }
    $t = [string](Get-Content -Raw -LiteralPath $p)
    if ($null -eq $t) { $t = '' }
    if ($t.Length -gt $max) { return $t.Substring(0,$max) + "`n...<truncated>" }
    return $t
}
$pass = @($results | Where-Object status -eq 'pass').Count
$fail = @($results | Where-Object status -eq 'fail').Count
$blocked = @($results | Where-Object status -eq 'blocked').Count
$rows = foreach ($r in $results) {
    $outText = HtmlEncode (Read-Short $r.stdout)
    $errText = HtmlEncode (Read-Short $r.stderr)
    $checksText = HtmlEncode (($r.checks | ConvertTo-Json -Compress -Depth 5))
    "<tr class='$($r.status)'><td>$($r.category)</td><td>$($r.name)</td><td>$($r.status)</td><td>$($r.exitCode)</td><td>$(HtmlEncode $r.notes)<details><summary>checks/log excerpts</summary><pre>$checksText</pre><h4>stdout</h4><pre>$outText</pre><h4>stderr</h4><pre>$errText</pre></details></td></tr>"
}
$html = @"
<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>OxVba External Integration E2E Showcase $RunId</title>
<style>
body{font-family:Segoe UI,Arial,sans-serif;margin:28px;line-height:1.35;color:#172033} code,pre{font-family:Cascadia Mono,Consolas,monospace} .hero{border-left:6px solid #315efb;background:#f4f7ff;padding:16px 20px;margin-bottom:20px} .grid{display:grid;grid-template-columns:repeat(4,1fr);gap:10px}.card{background:#f7f7f8;border:1px solid #ddd;border-radius:8px;padding:10px} table{border-collapse:collapse;width:100%;font-size:13px} th,td{border:1px solid #ddd;padding:7px;vertical-align:top} th{background:#172033;color:white}.pass td:nth-child(3){color:#078233;font-weight:700}.fail td:nth-child(3){color:#b00020;font-weight:700}.blocked td:nth-child(3){color:#8a5a00;font-weight:700} pre{white-space:pre-wrap;background:#0b1020;color:#e7edf7;padding:10px;border-radius:6px;max-height:260px;overflow:auto} .note{font-size:12px;color:#4b5563}
</style></head>
<body>
<div class="hero"><h1>OxVba external integration end-to-end showcase</h1><p><b>Run:</b> $RunId &nbsp; <b>Generated UTC:</b> $($summary.generatedUtc)</p><p>This is a live technical tour through OxVba's external interfaces: CLI compile/run, serialized bundle execution, project loading, VM execution, disabled-JIT diagnostics, Windows COM Automation against Access/ACE/Jet database components, and the bounded Immediate Window interface. The evidence below is intentionally executable and inspectable: sources, logs, .oxb bundles, and the generated .accdb sit under <code>$(HtmlEncode $runDir)</code>.</p></div>
<div class="grid"><div class="card"><b>Pass</b><br><span style="font-size:28px">$pass</span></div><div class="card"><b>Fail</b><br><span style="font-size:28px">$fail</span></div><div class="card"><b>Blocked</b><br><span style="font-size:28px">$blocked</span></div><div class="card"><b>Artifacts</b><br><code>summary.json</code>, logs, sources, bundles, accdb</div></div>
<h2>Executive technical readout</h2>
<ul>
<li><b>Compiler and bundle path:</b> a new single-file program was compiled to an OxBundle, the artifact was size-checked, source execution returned <code>42</code> plus a string sentinel, and the launcher consumed the compiled bundle.</li>
<li><b>Project model:</b> a fresh two-file project proved module roster loading, qualified cross-module calls, local procedure calls, string coercion/concatenation, VM target execution, explicit disabled-JIT diagnostics, and build serialization.</li>
<li><b>Diagnostics:</b> deliberate editing mistakes were preserved as non-zero external command failures with compiler/runtime diagnostics in stderr logs.</li>
<li><b>Access/Jet COM:</b> where ADOX/ADODB are installed, OxVba activated real COM objects, created an Access/ACE <code>.accdb</code>, created a table, inserted records, selected a row, traversed recordset fields through <code>DispatchInvoke</code>, and surfaced <code>Grace / 99</code> in the runtime snapshot. This is a live environment-dependent integration, not a mock.</li>
<li><b>Access/Jet mixed imported COM:</b> a second database pass imports real ADO/ADOX type libraries, declares <code>Dim catalog As New ADOX.Catalog</code> and <code>Dim cn As New ADODB.Connection</code>, drives <code>ADODB.Connection.Open/Execute</code> through metadata-backed calls, and explicitly remains mixed because unsupported pieces still use <code>DispatchInvoke</code>.</li>
<li><b>Access/Jet strict natural early binding:</b> a third pass uses no <code>DispatchInvoke</code> in the VBA source: it declares <code>ADOX.Catalog</code>, <code>ADODB.Connection</code>, <code>ADODB.Recordset</code>, and <code>ADODB.Field</code>, then exercises <code>rs.Fields("Name").Value</code> plus <code>rs!Name</code>/<code>rs!Score</code>.</li>
<li><b>Access/Jet DAO object model:</b> a late-bound / mixed / strict early-bound trio drives the <code>DAO.DBEngine</code> object model (<code>CreateDatabase</code> &rarr; <code>Execute</code> DDL/inserts &rarr; <code>OpenRecordset</code> &rarr; <code>Fields</code>) against a real Access/ACE <code>.accdb</code>. DAO objects are dispinterfaces obtained from method calls (not coclasses) and the DBEngine CLSID has no TypeLib registry association, so this proves interface-scoped binding and combined method/property-get dispatch beyond the ADO coclass shape; the strict pass uses natural <code>DAO.DBEngine</code>/<code>Database</code>/<code>Recordset</code>/<code>Field</code> types with no <code>DispatchInvoke</code>.</li>
<li><b>Immediate interface:</b> the bounded REPL evaluated project procedures, switched default modules, reset the live runtime session, and emitted a transcript suitable for IDE embedding discussions.</li>
</ul>
<h2>Truth boundaries for Q&amp;A</h2>
<p>This showcase does not claim full native AOT compilation or full Office/VBA COM parity. Current executable truth is bytecode plus VM execution; the old JIT v1 path is disabled pending a replacement design. COM traffic here uses the supported late-bound <code>CreateObject</code>/<code>DispatchInvoke</code> bridge plus a mixed imported-COM subset for ADO/ADOX member calls. The strict Access/JET pass now proves natural source syntax for the bounded ADO/ADOX slice, while broader descriptor-backed ABI unification remains in progress. Access/ACE availability is machine-dependent and recorded as blocked rather than fabricated if missing.</p>
<h2>Result matrix and log excerpts</h2>
<table><thead><tr><th>Area</th><th>Pass</th><th>Status</th><th>Exit</th><th>Evidence</th></tr></thead><tbody>
$($rows -join "`n")
</tbody></table>
<p class="note">Source root: <code>$(HtmlEncode $srcDir)</code>. Artifact root: <code>$(HtmlEncode $artifactDir)</code>. Log root: <code>$(HtmlEncode $logDir)</code>.</p>
</body></html>
"@
$htmlPath = Join-Path $runDir 'showcase.html'
Write-Utf8NoBom $htmlPath $html
Write-Host "summary=$summaryPath"
Write-Host "showcase=$htmlPath"
if ($fail -gt 0) { exit 1 }
exit 0
