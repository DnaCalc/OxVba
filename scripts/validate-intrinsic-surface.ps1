$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

Push-Location (Join-Path $PSScriptRoot "..")
try {
    $path = "docs/evidence/runtime/INTRINSIC_SURFACE.csv"
    if (-not (Test-Path $path)) {
        throw "Missing intrinsic surface file: $path"
    }

    $rows = Import-Csv $path
    if (-not $rows -or $rows.Count -eq 0) {
        throw "Intrinsic surface file is empty: $path"
    }

    $seen = @{}
    foreach ($row in $rows) {
        if ([string]::IsNullOrWhiteSpace($row.intrinsic)) {
            throw "Intrinsic surface row is missing intrinsic name"
        }
        $name = $row.intrinsic.Trim().ToLowerInvariant()
        if ($seen.ContainsKey($name)) {
            throw "Duplicate intrinsic entry in intrinsic surface file: $name"
        }
        $seen[$name] = $true

        if ($row.surface -notin @("deterministic-core", "host-sensitive")) {
            throw "Invalid intrinsic surface classification for ${name}: $($row.surface)"
        }

        if ([string]::IsNullOrWhiteSpace($row.min_arity)) {
            throw "Missing min_arity for intrinsic: ${name}"
        }

        [int]$minArity = 0
        if (-not [int]::TryParse($row.min_arity, [ref]$minArity) -or $minArity -lt 0) {
            throw "Invalid min_arity for intrinsic ${name}: $($row.min_arity)"
        }

        if ([string]::IsNullOrWhiteSpace($row.max_arity)) {
            throw "Missing max_arity for intrinsic: ${name}"
        }

        if ($row.max_arity -ne "variadic") {
            [int]$maxArity = 0
            if (-not [int]::TryParse($row.max_arity, [ref]$maxArity) -or $maxArity -lt $minArity) {
                throw "Invalid max_arity for intrinsic ${name}: $($row.max_arity)"
            }
        }
    }

    $hostSensitive = $rows |
        Where-Object { $_.surface -eq "host-sensitive" } |
        ForEach-Object { $_.intrinsic.Trim().ToLowerInvariant() } |
        Sort-Object

    $expectedHostSensitive = @("createobject", "dir", "dispatchinvoke", "environ", "shell")
    if (($hostSensitive -join ",") -ne ($expectedHostSensitive -join ",")) {
        throw "Host-sensitive intrinsic set mismatch. expected=$($expectedHostSensitive -join ',') actual=$($hostSensitive -join ',')"
    }

    Write-Host "intrinsic-surface: ok ($($rows.Count) entries)"
}
finally {
    Pop-Location
}
