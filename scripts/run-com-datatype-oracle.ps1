param(
    [string]$OutputRoot = "docs/evidence/conformance/oracle_captures",
    [string]$RunId = "",
    [switch]$NoArtifacts,
    [switch]$AllowMismatch
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not $IsWindows) { throw "COM datatype oracle runner is Windows-only" }
    . "$PSScriptRoot/lib-run-context.ps1"
    $resolvedRunId = Resolve-RunId -Name "com-datatype-oracle" -RequestedRunId $RunId
    if ($NoArtifacts) { $OutputRoot = New-NoArtifactEvidenceDir -Scope "com-datatype-oracle" -RunId $resolvedRunId }

    $workspaceRoot = (Resolve-Path ".").Path
    $runRoot = if ([System.IO.Path]::IsPathRooted($OutputRoot)) { $OutputRoot } else { Join-Path $workspaceRoot $OutputRoot }
    $runDir = Join-Path $runRoot "com_datatype_oracle_$resolvedRunId"
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null

    & (Join-Path $workspaceRoot "tools/OxVba.TestEventServer/register.ps1") -Configuration Debug -Scope CurrentUser

    $cases = @(
        @{ id="COM-DT-SCALAR-I1"; kind="scalar"; expr="obj.ReturnSignedByteObject()"; ox='DispatchInvoke(obj, "ReturnSignedByteObject")' },
        @{ id="COM-DT-SCALAR-UI2"; kind="scalar"; expr="obj.ReturnUnsignedWordObject()"; ox='DispatchInvoke(obj, "ReturnUnsignedWordObject")' },
        @{ id="COM-DT-SCALAR-UI4"; kind="scalar"; expr="obj.ReturnUnsignedLongObject()"; ox='DispatchInvoke(obj, "ReturnUnsignedLongObject")' },
        @{ id="COM-DT-SCALAR-UI8"; kind="scalar"; expr="obj.ReturnUnsignedHyperObject()"; ox='DispatchInvoke(obj, "ReturnUnsignedHyperObject")' },
        @{ id="COM-DT-SCALAR-DECIMAL"; kind="scalar"; expr="obj.ReturnDecimalObject()"; ox='DispatchInvoke(obj, "ReturnDecimalObject")' },
        @{ id="COM-DT-ARRAY-I1"; kind="array"; expr="obj.ReturnSignedByteArray()"; ox='DispatchInvoke(obj, "ReturnSignedByteArray")' },
        @{ id="COM-DT-ARRAY-UI2"; kind="array"; expr="obj.ReturnUnsignedWordArray()"; ox='DispatchInvoke(obj, "ReturnUnsignedWordArray")' },
        @{ id="COM-DT-ARRAY-UI4"; kind="array"; expr="obj.ReturnUnsignedLongArray()"; ox='DispatchInvoke(obj, "ReturnUnsignedLongArray")' },
        @{ id="COM-DT-ARRAY-UI8"; kind="array"; expr="obj.ReturnUnsignedHyperArray()"; ox='DispatchInvoke(obj, "ReturnUnsignedHyperArray")' },
        @{ id="COM-DT-ARRAY-DECIMAL"; kind="array"; expr="obj.ReturnDecimalArray()"; ox='DispatchInvoke(obj, "ReturnDecimalArray")' }
    )

    function Invoke-ExcelProbe {
        param($Case)
        $excel = $null; $wb = $null
        try {
            $excel = New-Object -ComObject Excel.Application
            $excel.Visible = $false; $excel.DisplayAlerts = $false
            $wb = $excel.Workbooks.Add()
            $mod = $wb.VBProject.VBComponents.Add(1)
            $mod.Name = "MainModule"
            $body = if ($Case.kind -eq "array") {
                'RunProbe = CStr(VarType(v)) & ":" & CStr(LBound(v)) & ":" & CStr(UBound(v)) & ":" & CStr(v(LBound(v))) & ":" & CStr(v(UBound(v)))'
            } else {
                'RunProbe = CStr(VarType(v)) & ":" & CStr(v)'
            }
            $code = @"
Public Function RunProbe()
On Error GoTo EH
Dim obj As Object, v As Variant
Set obj = CreateObject("OxVba.TestEventServer")
v = $($Case.expr)
$body
Exit Function
EH:
RunProbe = "ERR:" & CStr(Err.Number) & ":" & Err.Description
End Function
"@
            [void]$mod.CodeModule.AddFromString($code)
            @{ status="ok"; observed=[string]$excel.Run("RunProbe") }
        } catch {
            @{ status="error"; observed=$_.Exception.Message }
        } finally {
            if ($wb -ne $null) { $wb.Close($false); [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb) }
            if ($excel -ne $null) { $excel.Quit(); [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel) }
        }
    }

    function Invoke-OxProbe {
        param($Case)
        $sourcePath = Join-Path $runDir ($Case.id + ".bas")
        $describe = if ($Case.id -eq "COM-DT-ARRAY-DECIMAL") {
            'observed = CStr(VarType(v)) & ":" & CStr(LBound(v)) & ":" & CStr(UBound(v)) & ":" & CStr(v(LBound(v))) & ":" & CStr(v(UBound(v)))'
        } elseif ($Case.kind -eq "array") {
            'observed = CStr(VarType(v)) & ":" & CStr(LBound(v)) & ":" & CStr(UBound(v))'
        } else {
            'observed = CStr(VarType(v)) & ":" & CStr(v)'
        }
        $source = @"
Sub Main()
Dim obj
Dim v
Dim observed
obj = CreateObject("OxVba.TestEventServer")
v = $($Case.ox)
$describe
End Sub
"@
        Set-Content -Path $sourcePath -Value $source
        $logPath = Join-Path $runDir ($Case.id + ".oxvba.log.txt")
        $cargoArgs = @("run", "-p", "oxvba-cli", "--", "run", $sourcePath, "--dump-values", "--allow-com-activation", "true", "--runtime-class", "host-native")
        $cargoOutput = & cargo @cargoArgs 2>&1 | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE
        if ($exitCode -ne 0) { return @{ status="error"; observed="lane-failed(exit=$exitCode)"; log=$logPath } }
        $line = @($cargoOutput | Where-Object { $_ -like "VALUES:*" } | Select-Object -Last 1)
        if ($line.Count -eq 0) { return @{ status="error"; observed="missing VALUES output"; log=$logPath } }
        $payload = ([string]$line[0]).Substring("VALUES:".Length)
        $parts = $payload -split '\|'
        $observed = $parts[-1]
        if ($observed -like "string:*") { $observed = $observed.Substring("string:".Length).Trim('"') }
        @{ status="ok"; observed=$observed; log=$logPath }
    }

    $rows = foreach ($case in $cases) {
        $vba = Invoke-ExcelProbe -Case $case
        $ox = Invoke-OxProbe -Case $case
        [PSCustomObject]@{
            topic_id = "CCT-COM-DATATYPE"
            case_id = $case.id
            scenario = "Late-bound COM $($case.kind) VarType/value parity"
            vba_status = $vba.status
            vba_observed = $vba.observed
            oxvba_status = $ox.status
            oxvba_observed = $ox.observed
            match = if ($vba.status -eq $ox.status -and $vba.observed -eq $ox.observed) { "true" } else { "false" }
            notes = "Fixture: OxVba.TestEventServer; OxVba log=$($ox.log)"
        }
    }

    $csvPath = Join-Path $runDir "results.csv"
    $summaryPath = Join-Path $runDir "summary.md"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation
    $summary = @(
        "# COM Datatype Oracle Run", "", "- Run ID: $resolvedRunId", "- Generated UTC: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))", "- Fixture: `OxVba.TestEventServer` datatype methods", "- Results CSV: $csvPath", "- Match count: $((@($rows | Where-Object { $_.match -eq 'true' })).Count)", "- Mismatch count: $((@($rows | Where-Object { $_.match -ne 'true' })).Count)", "", "| Topic | Case | VBA | OxVba | Match | Notes |", "|---|---|---|---|---|---|"
    )
    foreach ($row in $rows) { $summary += "| $($row.topic_id) | $($row.case_id) | $($row.vba_status): $($row.vba_observed) | $($row.oxvba_status): $($row.oxvba_observed) | $($row.match) | $($row.notes) |" }
    Set-Content -Path $summaryPath -Value ($summary -join [Environment]::NewLine)
    Write-Host "com-datatype-oracle: complete"; Write-Host "run_dir=$runDir"; Write-Host "results=$csvPath"; Write-Host "summary=$summaryPath"
    if (-not $AllowMismatch -and (@($rows | Where-Object { $_.match -ne "true" }).Count -gt 0)) { exit 1 }
}
finally { Pop-Location }
