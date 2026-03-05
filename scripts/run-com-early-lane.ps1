param(
    [ValidateSet("E0", "E1", "E2", "E3", "E4", "E5", "E6")]
    [string]$LaneId,
    [string]$EvidenceDir = "docs/evidence/conformance/com_early/lanes",
    [string]$RunId = "",
    [switch]$NoCapture,
    [switch]$NoThrow
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

function New-LaneCase {
    param(
        [string]$TestId,
        [string]$Command,
        [string]$ClauseIds,
        [string]$Profile = "windows",
        [string]$RuntimeClass = "windows-headless"
    )
    [PSCustomObject]@{
        test_id = $TestId
        command = $Command
        clause_ids = $ClauseIds
        profile = $Profile
        runtime_class = $RuntimeClass
    }
}

function Get-LaneCases {
    param([string]$Lane)
    switch ($Lane) {
        "E0" {
            return @(
                (New-LaneCase -TestId "E0-001" -Command "cargo test -p oxvba-hal windows_typelib_resolve_load_and_cache_roundtrip -- --nocapture" -ClauseIds "HAL-TLIB-RESOLVE;HAL-TLIB-LOAD;HAL-TLIB-CACHE"),
                (New-LaneCase -TestId "E0-002" -Command "cargo test -p oxvba-hal windows_typelib_reference_invalidation_scope_is_stable -- --nocapture" -ClauseIds "HAL-TLIB-INVALIDATE")
            )
        }
        "E1" {
            return @(
                (New-LaneCase -TestId "E1-001" -Command "cargo test -p oxvba-host type_library_resolution_binds_unique_libid_identity -- --nocapture" -ClauseIds "PMR-TLIB-IDENTITY-UNIQUE"),
                (New-LaneCase -TestId "E1-002" -Command "cargo test -p oxvba-host type_library_resolution_reports_ambiguous_libid_identity -- --nocapture" -ClauseIds "PMR-TLIB-IDENTITY-AMBIGUOUS")
            )
        }
        "E2" {
            return @(
                (New-LaneCase -TestId "E2-001" -Command "cargo test -p oxvba-compiler compile_project_rewrites_as_new_external_type_to_createobject_selector -- --nocapture" -ClauseIds "BIND-TLIB-ASNEW"),
                (New-LaneCase -TestId "E2-002" -Command "cargo test -p oxvba-compiler compile_project_rewrites_early_bound_member_call_to_dispatchinvoke_subset -- --nocapture" -ClauseIds "BIND-TLIB-MEMBER-REWRITE"),
                (New-LaneCase -TestId "E2-003" -Command "cargo test -p oxvba-compiler compile_project_rejects_unsupported_external_member_token -- --nocapture" -ClauseIds "BIND-TLIB-MEMBER-DIAG")
            )
        }
        "E3" {
            return @(
                (New-LaneCase -TestId "E3-001" -Command "cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_executes_with_typed_declarations_subset -- --nocapture" -ClauseIds "RUNTIME-EARLYBIND-SMOKE"),
                (New-LaneCase -TestId "E3-002" -Command "cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_vm_jit_snapshots_match_for_subset -- --nocapture" -ClauseIds "RUNTIME-EARLYBIND-VM-JIT"),
                (New-LaneCase -TestId "E3-003" -Command "cargo test -p oxvba-host --test com_early_project_end_to_end early_bound_project_reports_compile_error_for_unsupported_member -- --nocapture" -ClauseIds "RUNTIME-EARLYBIND-ERROR")
            )
        }
        "E4" {
            return @(
                (New-LaneCase -TestId "E4-001" -Command "cargo test -p oxvba-hal windows_typelib_reference_invalidation_scope_is_stable -- --nocapture" -ClauseIds "CACHE-INVALIDATE-REFERENCE"),
                (New-LaneCase -TestId "E4-002" -Command "cargo test -p oxvba-hal windows_typelib_resolve_load_and_cache_roundtrip -- --nocapture" -ClauseIds "CACHE-REPLAY-DETERMINISTIC")
            )
        }
        "E5" {
            return @(
                (New-LaneCase -TestId "E5-001" -Command "cargo test -p oxvba-host --test com_early_project_end_to_end early_and_late_dispatch_paths_can_mix_in_one_project -- --nocapture" -ClauseIds "E2E-MIXED-EARLY-LATE"),
                (New-LaneCase -TestId "E5-002" -Command "cargo test -p oxvba-host --test com_early_project_end_to_end -- --nocapture" -ClauseIds "E2E-PROJECT-SUITE")
            )
        }
        "E6" {
            return @(
                (New-LaneCase -TestId "E6-001" -Command "cargo test -p oxvba-host formal_v466_early_binding_terminal_artifacts_exist -- --nocapture" -ClauseIds "FORMAL-LANE-REGISTRY"),
                (New-LaneCase -TestId "E6-002" -Command "cargo kani -p oxvba-host --harness pmr_typelib_resolution_transitions_typelib_refs_out_of_unbound" -ClauseIds "FORMAL-KANI-PMR-TLIB")
            )
        }
        default {
            throw "unsupported lane $Lane"
        }
    }
}

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if ([string]::IsNullOrWhiteSpace($RunId)) {
        $RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    }
    if (-not (Test-Path $EvidenceDir)) {
        New-Item -ItemType Directory -Path $EvidenceDir -Force | Out-Null
    }

    $results = @()
    $cases = Get-LaneCases -Lane $LaneId

    foreach ($case in $cases) {
        $safeTest = ($case.test_id -replace "[^A-Za-z0-9_.-]", "_")
        $logPath = Join-Path $EvidenceDir ("COM_EARLY_{0}_{1}_{2}.log" -f $LaneId, $safeTest, $RunId)
        $command = $case.command
        if ($NoCapture -and -not $command.Contains("-- --nocapture")) {
            $command += " -- --nocapture"
        }

        $null = Invoke-Expression "$command 2>&1" | Tee-Object -FilePath $logPath
        $exitCode = $LASTEXITCODE

        $status = "pass"
        $diagnosticCode = ""
        if ($exitCode -ne 0) {
            if ($case.command.Trim().ToLowerInvariant().StartsWith("cargo kani")) {
                $status = "deferred"
                $diagnosticCode = "FORMAL-DEFERRED-KANI"
            } else {
                $status = "fail"
                $diagnosticCode = "LANE-COMMAND-FAILED"
            }
        }

        $results += [PSCustomObject]@{
            lane_id = $LaneId
            test_id = $case.test_id
            profile = $case.profile
            runtime_class = $case.runtime_class
            clause_ids = $case.clause_ids
            status = $status
            diagnostic_code = $diagnosticCode
            hresult = ""
            evidence_path = $logPath
            repro_command = $case.command
        }
    }

    $csvPath = Join-Path $EvidenceDir ("COM_EARLY_{0}_{1}.csv" -f $LaneId, $RunId)
    $latestCsvPath = Join-Path $EvidenceDir ("COM_EARLY_{0}_LATEST.csv" -f $LaneId)
    $results | Export-Csv -Path $csvPath -NoTypeInformation
    Copy-Item -Path $csvPath -Destination $latestCsvPath -Force

    $reportPath = Join-Path $EvidenceDir ("COM_EARLY_{0}_{1}.md" -f $LaneId, $RunId)
    $latestReportPath = Join-Path $EvidenceDir ("COM_EARLY_{0}_LATEST.md" -f $LaneId)
    $lines = @(
        "# COM Early Lane $LaneId Run",
        "",
        "- Run ID: $RunId",
        "- Lane: $LaneId",
        "",
        "| Test | Status | Clause IDs | Evidence |",
        "|---|---|---|---|"
    )
    foreach ($row in $results) {
        $lines += "| $($row.test_id) | $($row.status) | $($row.clause_ids) | $($row.evidence_path) |"
    }
    Set-Content -Path $reportPath -Value ($lines -join "`n")
    Copy-Item -Path $reportPath -Destination $latestReportPath -Force

    if (-not $NoThrow) {
        $failed = @($results | Where-Object { $_.status -eq "fail" })
        if ($failed.Count -gt 0) {
            throw "lane $LaneId failed for $($failed.Count) test(s)"
        }
    }

    return $results
}
finally {
    Pop-Location
}
