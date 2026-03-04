$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $catalogPath = "conformance/integration/catalog.psv"
    $casesRoot = "conformance/integration/projects"
    if (-not (Test-Path $catalogPath)) {
        throw "Missing integration catalog: $catalogPath"
    }

    $lines = Get-Content $catalogPath | Where-Object {
        $trimmed = $_.Trim()
        $trimmed -ne "" -and -not $trimmed.StartsWith("#")
    }

    if ($lines.Count -lt 2) {
        throw "Integration catalog contains no data rows"
    }

    $allowedStatus = @("active", "active-limit", "deferred", "planned")
    $allowedBackend = @("vm", "jit", "both")
    $allowedExpectedStatus = @("ok", "error")
    $allowedExpectedPhase = @("any", "compile-time", "runtime")

    $header = $lines[0].Trim()
    $expectedHeader = "case_id|level|title|status|backend|runtime_profile|policy_preset|policy_overrides|unsupported_mode|expect_status|expect_phase|expect_slots|expect_error_contains|reference_order|deferred_gate|topic_refs|project_name|notes"
    if ($header -ne $expectedHeader) {
        throw "Unexpected catalog header. Expected: $expectedHeader"
    }

    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    $activeCount = 0
    $deferredCount = 0

    for ($i = 1; $i -lt $lines.Count; $i++) {
        $lineNo = $i + 1
        $parts = $lines[$i].Split('|')
        if ($parts.Count -ne 18) {
            throw "catalog line $lineNo has $($parts.Count) columns (expected 18)"
        }

        $caseId = $parts[0].Trim()
        if ([string]::IsNullOrWhiteSpace($caseId)) {
            throw "catalog line $lineNo has empty case_id"
        }
        if (-not $seen.Add($caseId)) {
            throw "duplicate case_id detected: $caseId"
        }

        $status = $parts[3].Trim().ToLowerInvariant()
        $backend = $parts[4].Trim().ToLowerInvariant()
        $expectStatus = $parts[9].Trim().ToLowerInvariant()
        $expectPhase = $parts[10].Trim().ToLowerInvariant()
        $deferredGate = $parts[14].Trim()
        $topicRefs = $parts[15].Trim()
        $projectName = $parts[16].Trim()

        if ($allowedStatus -notcontains $status) {
            throw "catalog line $lineNo has invalid status: $status"
        }
        if ($allowedBackend -notcontains $backend) {
            throw "catalog line $lineNo has invalid backend: $backend"
        }
        if ($allowedExpectedStatus -notcontains $expectStatus) {
            throw "catalog line $lineNo has invalid expect_status: $expectStatus"
        }
        if ($allowedExpectedPhase -notcontains $expectPhase) {
            throw "catalog line $lineNo has invalid expect_phase: $expectPhase"
        }
        if ([string]::IsNullOrWhiteSpace($projectName)) {
            throw "catalog line $lineNo has empty project_name"
        }

        if ($status -eq "active" -or $status -eq "active-limit") {
            $activeCount++
            $caseMainDir = Join-Path $casesRoot "$caseId/main"
            if (-not (Test-Path $caseMainDir)) {
                throw "active case $caseId is missing fixture dir: $caseMainDir"
            }
        }
        if ($status -eq "deferred" -or $status -eq "planned") {
            $deferredCount++
            if ([string]::IsNullOrWhiteSpace($deferredGate) -and [string]::IsNullOrWhiteSpace($topicRefs)) {
                throw "deferred/planned case $caseId must set deferred_gate and/or topic_refs"
            }
        }
    }

    if ($activeCount -le 0) {
        throw "integration catalog has zero active cases"
    }
    if ($deferredCount -le 0) {
        throw "integration catalog has zero deferred/planned cases"
    }

    Write-Host "project-integration-catalog: ok (active=$activeCount deferred=$deferredCount rows=$($lines.Count - 1))"
}
finally {
    Pop-Location
}
