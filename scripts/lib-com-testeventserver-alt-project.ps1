function Replace-InFile {
    param(
        [string]$Path,
        [string]$OldValue,
        [string]$NewValue
    )

    $content = Get-Content $Path -Raw
    $content = $content.Replace($OldValue, $NewValue)
    Set-Content -Path $Path -Value $content -Encoding UTF8
}

function New-AltTestEventServerProject {
    param(
        [string]$WorkspaceRoot,
        [string]$DestinationRoot
    )

    $sourceRoot = Join-Path $WorkspaceRoot "tools/OxVba.TestEventServer"
    if (Test-Path $DestinationRoot) {
        Remove-Item -Recurse -Force -Path $DestinationRoot
    }
    Copy-Item -Recurse -Force -Path $sourceRoot -Destination $DestinationRoot

    foreach ($buildDir in @("bin", "obj")) {
        $candidate = Join-Path $DestinationRoot $buildDir
        if (Test-Path $candidate) {
            Remove-Item -Recurse -Force -Path $candidate
        }
    }

    Rename-Item `
        -Path (Join-Path $DestinationRoot "OxVba.TestEventServer.csproj") `
        -NewName "OxVba.TestEventServerAlt.csproj"
    Rename-Item `
        -Path (Join-Path $DestinationRoot "OxVba.TestEventServer.hkcu.reg") `
        -NewName "OxVba.TestEventServerAlt.hkcu.reg"
    Rename-Item `
        -Path (Join-Path $DestinationRoot "OxVba.TestEventServer.reg") `
        -NewName "OxVba.TestEventServerAlt.reg"

    $files = @(
        (Join-Path $DestinationRoot "OxVba.TestEventServerAlt.csproj"),
        (Join-Path $DestinationRoot "OxVba.TestEventServerAlt.hkcu.reg"),
        (Join-Path $DestinationRoot "OxVba.TestEventServerAlt.reg"),
        (Join-Path $DestinationRoot "register.ps1"),
        (Join-Path $DestinationRoot "TestEventServer.cs")
    )
    foreach ($file in $files) {
        Replace-InFile -Path $file -OldValue "OxVba.TestEventServer" -NewValue "OxVba.TestEventServerAlt"
    }

    Replace-InFile `
        -Path (Join-Path $DestinationRoot "Properties/AssemblyInfo.cs") `
        -OldValue "E2A30001-0001-0001-0001-000000000001" `
        -NewValue "E2A30001-0001-0001-0001-000000000101"
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "E2A30001-0001-0001-0001-000000000002" `
        -NewValue "E2A30001-0001-0001-0001-000000000102"
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "E2A30001-0001-0001-0001-000000000003" `
        -NewValue "E2A30001-0001-0001-0001-000000000103"
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "E2A30001-0001-0001-0001-000000000004" `
        -NewValue "E2A30001-0001-0001-0001-000000000104"
    foreach ($registrationFile in @(
            (Join-Path $DestinationRoot "OxVba.TestEventServerAlt.hkcu.reg"),
            (Join-Path $DestinationRoot "OxVba.TestEventServerAlt.reg")
        )) {
        Replace-InFile `
            -Path $registrationFile `
            -OldValue "E2A30001-0001-0001-0001-000000000004" `
            -NewValue "E2A30001-0001-0001-0001-000000000104"
    }
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "Deterministic COM event test server for OxVba registered event lane parity." `
        -NewValue "Deterministic alt COM event test server for OxVba registered event lane parity."
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "OxVba.TestEventServerAlt.csproj") `
        -OldValue "Deterministic COM event test server for OxVba registered event lane parity." `
        -NewValue "Deterministic alt COM event test server for OxVba registered event lane parity."
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "return 42;" `
        -NewValue "return 84;"
}
