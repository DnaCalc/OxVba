$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $path = "docs/evidence/conformance/DEFERRED_ORACLE_GATES.csv"
    if (-not (Test-Path $path)) {
        throw "missing deferred oracle gate register: $path"
    }

    $rows = Import-Csv $path
    if ($rows.Count -eq 0) {
        throw "deferred oracle gate register is empty"
    }

    $expectedColumns = @(
        "gate_id",
        "topic_id",
        "domain",
        "track",
        "scope_class",
        "status",
        "owner_phase",
        "unblock_condition",
        "evidence",
        "foldback_required",
        "foldback_steps",
        "close_condition",
        "notes"
    )
    $actualColumns = @($rows[0].PSObject.Properties.Name)
    if ($actualColumns.Count -ne $expectedColumns.Count) {
        throw "deferred oracle gate register has $($actualColumns.Count) columns (expected $($expectedColumns.Count))"
    }
    for ($i = 0; $i -lt $expectedColumns.Count; $i++) {
        if ($actualColumns[$i] -ne $expectedColumns[$i]) {
            throw ("deferred oracle gate register column mismatch at index {0}: expected '{1}', got '{2}'" -f $i, $expectedColumns[$i], $actualColumns[$i])
        }
    }

    foreach ($row in $rows) {
        $row.gate_id = ([string]$row.gate_id).Trim()
        $row.topic_id = ([string]$row.topic_id).Trim()
        $row.scope_class = ([string]$row.scope_class).Trim()
        $row.status = ([string]$row.status).Trim()
        $row.foldback_required = ([string]$row.foldback_required).Trim().ToLowerInvariant()
        $row.foldback_steps = [string]$row.foldback_steps
        $row.close_condition = [string]$row.close_condition
        $row.notes = [string]$row.notes
    }

    $dupeGateIds = $rows | Group-Object gate_id | Where-Object { $_.Count -gt 1 }
    if ($dupeGateIds) {
        $ids = ($dupeGateIds | ForEach-Object { $_.Name }) -join ", "
        throw "duplicate gate_id entries found: $ids"
    }

    $dupeTopics = $rows | Group-Object topic_id | Where-Object { $_.Count -gt 1 }
    if ($dupeTopics) {
        $topics = ($dupeTopics | ForEach-Object { $_.Name }) -join ", "
        throw "duplicate topic_id entries found: $topics"
    }

    foreach ($row in $rows) {
        if ($row.gate_id -notmatch "^ODG-\d{3}$") {
            throw "invalid gate_id format: $($row.gate_id)"
        }

        if ($row.foldback_required -ne "true" -and $row.foldback_required -ne "false") {
            throw "invalid foldback_required value for $($row.gate_id): $($row.foldback_required)"
        }

        if ([string]::IsNullOrWhiteSpace($row.close_condition)) {
            throw "missing close_condition for $($row.gate_id)"
        }

        if ($row.foldback_required -eq "true" -and [string]::IsNullOrWhiteSpace($row.foldback_steps)) {
            throw "foldback_required=true but foldback_steps is empty: $($row.gate_id)"
        }

        if ($row.foldback_required -eq "false" -and -not [string]::IsNullOrWhiteSpace($row.foldback_steps)) {
            throw "foldback_required=false but foldback_steps is populated: $($row.gate_id)"
        }

        if ($row.scope_class -eq "non-hal" -and $row.status -eq "open") {
            if ($row.foldback_required -ne "true") {
                throw "non-hal open gate must set foldback_required=true: $($row.gate_id)"
            }
            if ([string]::IsNullOrWhiteSpace($row.foldback_steps)) {
                throw "non-hal open gate missing foldback_steps: $($row.gate_id)"
            }
        }
    }

    $openCount = ($rows | Where-Object { $_.status -eq "open" }).Count
    $nonHalOpen = ($rows | Where-Object { $_.status -eq "open" -and $_.scope_class -eq "non-hal" }).Count
    Write-Host "deferred-oracle-gates: ok (rows=$($rows.Count) open=$openCount non_hal_open=$nonHalOpen)"
}
finally {
    Pop-Location
}
