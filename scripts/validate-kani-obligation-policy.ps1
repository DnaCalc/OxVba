param(
    [string]$ObligationsPath = "docs/evidence/formal/obligations.csv",
    [string]$PolicyPath = "docs/evidence/formal/KANI_OBLIGATION_POLICY_V1.csv"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

if (-not (Test-Path -LiteralPath $ObligationsPath)) {
    throw "Missing obligations file: $ObligationsPath"
}
if (-not (Test-Path -LiteralPath $PolicyPath)) {
    throw "Missing Kani policy file: $PolicyPath"
}

$obligations = Import-Csv -LiteralPath $ObligationsPath | Where-Object {
    $_.active -eq "true" -and $_.command -like "cargo kani*"
}
$policy = Import-Csv -LiteralPath $PolicyPath

$requiredPolicyColumns = @(
    "obligation_id",
    "profile",
    "signal_tier",
    "primary_value",
    "current_state",
    "execution_policy",
    "next_action"
)
foreach ($column in $requiredPolicyColumns) {
    if ($policy.Count -gt 0 -and -not ($policy[0].PSObject.Properties.Name -contains $column)) {
        throw "Policy file missing required column: $column"
    }
}

$allowedTiers = @("high", "medium", "low")
$duplicatePolicy = $policy | Group-Object obligation_id | Where-Object { $_.Count -gt 1 }
if ($duplicatePolicy.Count -gt 0) {
    $ids = ($duplicatePolicy | ForEach-Object { $_.Name }) -join ", "
    throw "Duplicate obligation_id rows in policy: $ids"
}

$policyMap = @{}
foreach ($row in $policy) {
    $id = $row.obligation_id
    $policyMap[$id] = $row
    if ($allowedTiers -notcontains $row.signal_tier) {
        throw "Invalid signal_tier '$($row.signal_tier)' for obligation_id $id"
    }
}

$obligationIds = @($obligations | ForEach-Object { $_.obligation_id })
$missing = @($obligationIds | Where-Object { -not $policyMap.ContainsKey($_) })
if ($missing.Count -gt 0) {
    throw "Policy file missing active Kani obligations: $($missing -join ', ')"
}

$extra = @($policy | Where-Object { $_.obligation_id -notin $obligationIds } | ForEach-Object { $_.obligation_id })
if ($extra.Count -gt 0) {
    throw "Policy file contains obligations not in active Kani set: $($extra -join ', ')"
}

foreach ($ob in $obligations) {
    $policyRow = $policyMap[$ob.obligation_id]
    if ($policyRow.profile -ne $ob.profile) {
        throw "Profile mismatch for $($ob.obligation_id): policy=$($policyRow.profile), obligations=$($ob.profile)"
    }
}

Write-Host "kani-obligation-policy: ok (active=$($obligations.Count), policy_rows=$($policy.Count))"
