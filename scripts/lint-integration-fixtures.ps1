param(
    [string]$Root = "conformance/integration/projects"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not (Test-Path $Root)) {
        throw "integration fixture root missing: $Root"
    }

    $issues = New-Object System.Collections.Generic.List[object]
    $files = Get-ChildItem -Path $Root -Recurse -File -Filter *.bas
    foreach ($file in $files) {
        $lineNumber = 0
        foreach ($line in Get-Content $file.FullName) {
            $lineNumber++
            if ($line -match '^\s*If\s+.+\s+Then\s+\S+' -and $line -notmatch '^\s*If\s+.+\s+Then\s*$') {
                $issues.Add([PSCustomObject]@{
                    rule = "INT-LINT-001"
                    path = $file.FullName
                    line = $lineNumber
                    message = "single-line If form detected; prefer multiline If/End If for backend stability"
                })
            }
            if ($line -match '^\s*\w+\s*=\s*.+\+\s*[A-Za-z_]\w*\.[A-Za-z_]\w*\(') {
                $issues.Add([PSCustomObject]@{
                    rule = "INT-LINT-002"
                    path = $file.FullName
                    line = $lineNumber
                    message = "additive expression with project-qualified function call detected; split into intermediate assignment"
                })
            }
        }
    }

    if ($issues.Count -gt 0) {
        Write-Host "[oxvba] integration fixture lint failed: $($issues.Count) issue(s)"
        foreach ($issue in $issues) {
            Write-Host "[$($issue.rule)] $($issue.path):$($issue.line) - $($issue.message)"
        }
        exit 1
    }

    Write-Host "[oxvba] integration fixture lint passed ($($files.Count) files)"
}
finally {
    Pop-Location
}
