param(
    [string]$WorksetPath = "docs/worksets/WORKSET_2026-03-29_VALIDATION_MATRIX_RESET_AND_BEAD_EXECUTION_REFORM.md"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not (Test-Path $WorksetPath)) {
        throw "validate-workset-rollout: workset not found: $WorksetPath"
    }
    if (-not (Test-Path ".beads/issues.jsonl")) {
        throw "validate-workset-rollout: .beads/issues.jsonl not found"
    }

    $issues = Get-Content ".beads/issues.jsonl" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | ForEach-Object { $_ | ConvertFrom-Json }
    $issueById = @{}
    foreach ($issue in $issues) {
        $issueById[$issue.id] = $issue
    }

    $workset = Get-Content $WorksetPath -Raw

    $rolloutSectionMatch = [regex]::Match($workset, 'Current rollout state:\s*(?<body>[\s\S]*?)Current active unfinished execution lanes:')
    if (-not $rolloutSectionMatch.Success) {
        throw "validate-workset-rollout: could not find 'Current rollout state' section"
    }
    $rolloutIds = [regex]::Matches($rolloutSectionMatch.Groups["body"].Value, 'bd-gm3\.\d+') | ForEach-Object { $_.Value } | Sort-Object -Unique
    if ($rolloutIds.Count -eq 0) {
        throw "validate-workset-rollout: no epic ids found in workset rollout section"
    }

    foreach ($id in $rolloutIds) {
        if (-not $issueById.ContainsKey($id)) {
            throw "validate-workset-rollout: workset references missing bead id $id"
        }
        if ($issueById[$id].issue_type -ne "epic") {
            throw "validate-workset-rollout: referenced rollout id $id is not an epic"
        }
    }

    $childrenByParent = @{}
    foreach ($issue in $issues) {
        if ($issue.dependencies) {
            foreach ($dep in $issue.dependencies) {
                if ($dep.type -eq "parent-child") {
                    if (-not $childrenByParent.ContainsKey($dep.depends_on_id)) {
                        $childrenByParent[$dep.depends_on_id] = @()
                    }
                    $childrenByParent[$dep.depends_on_id] += $issue.id
                }
            }
        }
    }

    foreach ($id in $rolloutIds) {
        if (-not $childrenByParent.ContainsKey($id) -or $childrenByParent[$id].Count -eq 0) {
            throw "validate-workset-rollout: epic $id has no child work"
        }
    }

    $unfinishedSectionMatch = [regex]::Match($workset, 'Current active unfinished execution lanes:\s*(?<body>[\s\S]*)$')
    if (-not $unfinishedSectionMatch.Success) {
        throw "validate-workset-rollout: could not find 'Current active unfinished execution lanes' section"
    }
    $unfinishedIds = [regex]::Matches($unfinishedSectionMatch.Groups["body"].Value, 'bd-gm3\.\d+') | ForEach-Object { $_.Value } | Sort-Object -Unique
    foreach ($id in $unfinishedIds) {
        if (-not $issueById.ContainsKey($id)) {
            throw "validate-workset-rollout: unfinished lane id $id not found"
        }
        if ($issueById[$id].status -eq "closed") {
            throw "validate-workset-rollout: unfinished lane id $id is already closed"
        }
    }

    Write-Host "validate-workset-rollout: ok (epics=$($rolloutIds.Count) unfinished=$($unfinishedIds.Count))"
}
finally {
    Pop-Location
}
