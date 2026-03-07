param(
    [switch]$Check
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $manifestPath = "docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv"
    if (-not (Test-Path $manifestPath)) {
        throw "missing PMR event diagnostic manifest: $manifestPath"
    }

    $rows = Import-Csv $manifestPath
    if ($rows.Count -eq 0) {
        throw "PMR event diagnostic manifest is empty: $manifestPath"
    }

    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    $ordered = $rows | Sort-Object diag_id
    foreach ($row in $ordered) {
        $diagId = ([string]$row.diag_id).Trim()
        if ([string]::IsNullOrWhiteSpace($diagId)) {
            throw "manifest contains row with empty diag_id"
        }
        if (-not $seen.Add($diagId)) {
            throw "duplicate diag_id in manifest: $diagId"
        }
    }

    $activeLines = @(
        "<!-- generated: scripts/generate-pmr-event-diagnostic-snippets.ps1 -->",
        "# PMR Event Diagnostic IDs (Generated)",
        "",
        'Canonical source: `docs/evidence/diagnostics/PMR_EVENT_DIAGNOSTICS_V1.csv`.',
        "",
        "| Diagnostic ID | Phase | Status | Description |",
        "|---|---|---|---|"
    )

    $legacyIds = [System.Collections.Generic.List[string]]::new()
    foreach ($row in $ordered) {
        $diagId = ([string]$row.diag_id).Trim()
        $phase = ([string]$row.phase).Trim()
        $status = ([string]$row.status).Trim()
        $description = ([string]$row.description).Trim()
        $legacyRaw = ([string]$row.legacy_ids).Trim()
        $activeLines += ('| `{0}` | {1} | {2} | {3} |' -f $diagId, $phase, $status, $description)
        if (-not [string]::IsNullOrWhiteSpace($legacyRaw)) {
            foreach ($legacyId in ($legacyRaw -split ';')) {
                $trimmed = $legacyId.Trim()
                if (-not [string]::IsNullOrWhiteSpace($trimmed)) {
                    $legacyIds.Add($trimmed)
                }
            }
        }
    }

    $legacyUnique = $legacyIds | Sort-Object -Unique
    $legacyLines = @(
        "<!-- generated: scripts/generate-pmr-event-diagnostic-snippets.ps1 -->",
        "# PMR Event Legacy Diagnostic IDs (Generated)",
        "",
        'Legacy IDs may appear in historical captures under `docs/evidence/conformance/oracle_captures/`.',
        "Active docs and active integration catalogs must use the canonical IDs listed in:",
        '- `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md`',
        ""
    )
    if ($legacyUnique.Count -gt 0) {
        $legacyLines += "Legacy IDs:"
        foreach ($legacyId in $legacyUnique) {
            $legacyLines += ('- `{0}`' -f $legacyId)
        }
    }
    else {
        $legacyLines += "Legacy IDs: _none_"
    }

    $activeOutPath = "docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md"
    $legacyOutPath = "docs/generated/PMR_EVENT_DIAGNOSTIC_LEGACY_IDS.md"
    $diagIdListPath = "docs/generated/PMR_EVENT_DIAGNOSTIC_IDS.txt"
    $legacyIdListPath = "docs/generated/PMR_EVENT_DIAGNOSTIC_LEGACY_IDS.txt"

    $activeText = ($activeLines -join "`n") + "`n"
    $legacyText = ($legacyLines -join "`n") + "`n"
    $diagListText = (($ordered | ForEach-Object { ([string]$_.diag_id).Trim() }) -join "`n") + "`n"
    $legacyListText = (($legacyUnique | ForEach-Object { $_.Trim() }) -join "`n") + "`n"

    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    $targets = @(
        @{ Path = $activeOutPath; Text = $activeText },
        @{ Path = $legacyOutPath; Text = $legacyText },
        @{ Path = $diagIdListPath; Text = $diagListText },
        @{ Path = $legacyIdListPath; Text = $legacyListText }
    )

    foreach ($target in $targets) {
        $path = [string]$target.Path
        $text = [string]$target.Text
        if ($Check) {
            if (-not (Test-Path $path)) {
                throw "missing generated PMR diagnostic artifact: $path (run scripts/generate-pmr-event-diagnostic-snippets.ps1)"
            }
            $existing = [System.IO.File]::ReadAllText((Resolve-Path $path), [System.Text.Encoding]::UTF8)
            if ($existing -ne $text) {
                throw "stale generated PMR diagnostic artifact: $path (run scripts/generate-pmr-event-diagnostic-snippets.ps1)"
            }
        }
        else {
            $parent = Split-Path $path -Parent
            if (-not [string]::IsNullOrWhiteSpace($parent) -and -not (Test-Path $parent)) {
                New-Item -ItemType Directory -Path $parent -Force | Out-Null
            }
            $absolutePath = Join-Path (Get-Location) $path
            [System.IO.File]::WriteAllText($absolutePath, $text, $utf8NoBom)
        }
    }

    if ($Check) {
        Write-Host "pmr-event-diagnostics-snippets: ok (checked)"
    }
    else {
        Write-Host "pmr-event-diagnostics-snippets: generated ($activeOutPath, $legacyOutPath)"
    }
}
finally {
    Pop-Location
}
