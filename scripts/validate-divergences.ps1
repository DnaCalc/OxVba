$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $divergenceDir = "docs/evidence/divergences"
    if (-not (Test-Path $divergenceDir)) {
        throw "Missing divergence directory: $divergenceDir"
    }

    $requiredPrefixes = @(
        "- Scope impact:",
        "- Fixture:",
        "- Reproduction command:",
        "- Tracking status:"
    )

    $records = Get-ChildItem -Path $divergenceDir -Filter "DIV-*.md" | Sort-Object Name
    if ($records.Count -eq 0) {
        throw "No divergence records found under $divergenceDir"
    }

    foreach ($record in $records) {
        $text = Get-Content $record.FullName -Raw
        if (-not $text.StartsWith("# DIV-")) {
            throw "Divergence record missing title header: $($record.Name)"
        }

        foreach ($prefix in $requiredPrefixes) {
            if (-not ($text -match "(?m)^$([regex]::Escape($prefix))")) {
                throw "Divergence record missing required field '$prefix': $($record.Name)"
            }
        }
    }

    Write-Host "divergence-structure: ok ($($records.Count) records)"
}
finally {
    Pop-Location
}
