param(
    [string]$OutputDir = "docs/evidence/profiles/v147"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not (Test-Path $OutputDir)) {
        New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    }

    $coveragePath = "docs/evidence/language/COVERAGE_INDEX.csv"
    $libraryPath = "docs/evidence/runtime/LIBRARY_CHECKLIST.csv"
    $specPath = "docs/evidence/SPEC_CHECKLIST.md"

    $rows = New-Object System.Collections.Generic.List[object]

    function Get-ScopeClass([string]$text) {
        $t = $text.ToLowerInvariant()
        if (
            $t.Contains("host-sensitive") -or
            $t.Contains("shell") -or
            $t.Contains("environ") -or
            $t.Contains("dir") -or
            $t.Contains("createobject") -or
            $t.Contains("dispatch") -or
            $t.Contains("com") -or
            $t.Contains("external automation") -or
            $t.Contains("interaction/ui") -or
            $t.Contains("msgbox") -or
            $t.Contains("inputbox") -or
            $t.Contains("file-introspection") -or
            $t.Contains("freefile") -or
            $t.Contains("eof/lof/seek") -or
            $t.Contains("file i/o") -or
            $t.Contains("open/close") -or
            $t.Contains("line input") -or
            $t.Contains("print#/write#") -or
            $t.Contains("typelibrary") -or
            $t.Contains("type-library") -or
            $t.Contains("boundary marshalling")
        ) {
            return "hal-adjacent"
        }
        return "non-hal"
    }

    function Get-TargetProfile([string]$text, [string]$scopeClass) {
        if ($scopeClass -eq "hal-adjacent") {
            return "excluded-current"
        }
        $t = $text.ToLowerInvariant()
        if ($t.Contains("err")) { return "v148-v149" }
        if ($t.Contains("string bstr") -or $t.Contains("vbnullstring") -or $t.Contains("string")) {
            return "v150-v151"
        }
        if ($t.Contains("udt")) { return "v152" }
        if ($t.Contains("null/empty/error") -or $t.Contains("coercion")) { return "v153" }
        if ($t.Contains("financial") -or $t.Contains("irr") -or $t.Contains("rate") -or $t.Contains("nper")) {
            return "v154-v156"
        }
        if ($t.Contains("diagnostic")) { return "v157" }
        return "v147-triage"
    }

    if (Test-Path $coveragePath) {
        $coverage = Import-Csv $coveragePath
        foreach ($row in $coverage) {
            if ($row.status -notin @("partial", "planned")) { continue }
            $desc = "$($row.feature_area)::$($row.construct)"
            $scopeClass = Get-ScopeClass $desc
            $rows.Add([pscustomobject]@{
                    source = "coverage_index"
                    row_id = $desc
                    status = $row.status
                    scope_class = $scopeClass
                    target_profile = Get-TargetProfile $desc $scopeClass
                    evidence = $row.evidence
                    notes = $row.notes
                })
        }
    }

    if (Test-Path $libraryPath) {
        $library = Import-Csv $libraryPath
        foreach ($row in $library) {
            if ($row.status -notin @("partial", "planned")) { continue }
            $desc = "$($row.library_family)::$($row.function_or_surface)"
            $scopeClass = Get-ScopeClass $desc
            $rows.Add([pscustomobject]@{
                    source = "library_checklist"
                    row_id = $desc
                    status = $row.status
                    scope_class = $scopeClass
                    target_profile = Get-TargetProfile $desc $scopeClass
                    evidence = $row.evidence
                    notes = $row.notes
                })
        }
    }

    if (Test-Path $specPath) {
        $lines = Get-Content $specPath
        foreach ($line in $lines) {
            if ($line -notmatch '^\|\s*`\[(~| )\]`\s*\|') { continue }
            $parts = $line.Split('|')
            if ($parts.Count -lt 6) { continue }
            $specFamily = $parts[2].Trim()
            $feature = $parts[3].Trim()
            $scopeEvidence = $parts[4].Trim()
            $notes = $parts[5].Trim()
            $statusToken = if ($line.Contains('`[~]`')) { "partial" } else { "planned" }
            $desc = "$specFamily::$feature"
            $scopeClass = Get-ScopeClass $desc
            $rows.Add([pscustomobject]@{
                    source = "spec_checklist"
                    row_id = $desc
                    status = $statusToken
                    scope_class = $scopeClass
                    target_profile = Get-TargetProfile $desc $scopeClass
                    evidence = $scopeEvidence
                    notes = $notes
                })
        }
    }

    $rows = $rows | Sort-Object source, row_id
    $csvPath = Join-Path $OutputDir "non_hal_gap_baseline.csv"
    $rows | Export-Csv -Path $csvPath -NoTypeInformation

    $total = $rows.Count
    $nonHal = ($rows | Where-Object { $_.scope_class -eq "non-hal" }).Count
    $halAdjacent = ($rows | Where-Object { $_.scope_class -eq "hal-adjacent" }).Count

    $countsBySource = $rows | Group-Object source | Sort-Object Name
    $countsByTarget = $rows | Group-Object target_profile | Sort-Object Name

    $md = New-Object System.Collections.Generic.List[string]
    $md.Add("# v147 Non-HAL Gap Baseline")
    $md.Add("")
    $md.Add("- Generated (UTC): $((Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ"))")
    $md.Add("- Source files:")
    $md.Add('  - `docs/evidence/SPEC_CHECKLIST.md`')
    $md.Add('  - `docs/evidence/language/COVERAGE_INDEX.csv`')
    $md.Add('  - `docs/evidence/runtime/LIBRARY_CHECKLIST.csv`')
    $md.Add("- Rows captured: $total")
    $md.Add("- Non-HAL rows: $nonHal")
    $md.Add("- HAL-adjacent rows: $halAdjacent")
    $md.Add("")
    $md.Add("## Counts by Source")
    $md.Add("")
    $md.Add("| Source | Count |")
    $md.Add("|---|---|")
    foreach ($g in $countsBySource) {
        $md.Add("| $($g.Name) | $($g.Count) |")
    }
    $md.Add("")
    $md.Add("## Counts by Target Profile")
    $md.Add("")
    $md.Add("| Target profile | Count |")
    $md.Add("|---|---|")
    foreach ($g in $countsByTarget) {
        $md.Add("| $($g.Name) | $($g.Count) |")
    }
    $md.Add("")
    $md.Add("## Artifact")
    $md.Add("")
    $md.Add('- `non_hal_gap_baseline.csv`')

    $mdPath = Join-Path $OutputDir "non_hal_gap_baseline.md"
    Set-Content -Path $mdPath -Value ($md -join "`n")

    Write-Host "gap baseline generated: $csvPath"
    Write-Host "summary generated: $mdPath"
}
finally {
    Pop-Location
}
