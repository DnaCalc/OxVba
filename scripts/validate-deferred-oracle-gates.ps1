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

    foreach ($row in $rows) {
        $row.gate_id = ([string]$row.gate_id).Trim()
        $row.topic_id = ([string]$row.topic_id).Trim()
        $row.scope_class = ([string]$row.scope_class).Trim()
        $row.status = ([string]$row.status).Trim()
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

        if ($row.scope_class -eq "non-hal" -and $row.status -eq "open") {
            if (-not $row.notes.Contains("Foldback:")) {
                throw "non-hal open gate missing Foldback note: $($row.gate_id)"
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
