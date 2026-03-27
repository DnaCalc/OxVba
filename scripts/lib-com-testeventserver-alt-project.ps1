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

function New-TestEventServerVariantProject {
    param(
        [string]$WorkspaceRoot,
        [string]$DestinationRoot,
        [string]$ProjectName,
        [string]$LibraryGuid,
        [string]$EventsGuid,
        [string]$ClassGuid,
        [string]$InterfaceGuid,
        [int]$PingValue,
        [string]$Description
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
        -NewName "$ProjectName.csproj"
    Rename-Item `
        -Path (Join-Path $DestinationRoot "OxVba.TestEventServer.hkcu.reg") `
        -NewName "$ProjectName.hkcu.reg"
    Rename-Item `
        -Path (Join-Path $DestinationRoot "OxVba.TestEventServer.reg") `
        -NewName "$ProjectName.reg"

    $files = @(
        (Join-Path $DestinationRoot "$ProjectName.csproj"),
        (Join-Path $DestinationRoot "$ProjectName.hkcu.reg"),
        (Join-Path $DestinationRoot "$ProjectName.reg"),
        (Join-Path $DestinationRoot "register.ps1"),
        (Join-Path $DestinationRoot "TestEventServer.cs")
    )
    foreach ($file in $files) {
        Replace-InFile -Path $file -OldValue "OxVba.TestEventServer" -NewValue $ProjectName
    }

    Replace-InFile `
        -Path (Join-Path $DestinationRoot "Properties/AssemblyInfo.cs") `
        -OldValue "E2A30001-0001-0001-0001-000000000001" `
        -NewValue $LibraryGuid
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "E2A30001-0001-0001-0001-000000000002" `
        -NewValue $EventsGuid
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "E2A30001-0001-0001-0001-000000000003" `
        -NewValue $ClassGuid
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "E2A30001-0001-0001-0001-000000000004" `
        -NewValue $InterfaceGuid
    foreach ($registrationFile in @(
            (Join-Path $DestinationRoot "$ProjectName.hkcu.reg"),
            (Join-Path $DestinationRoot "$ProjectName.reg")
        )) {
        Replace-InFile `
            -Path $registrationFile `
            -OldValue "E2A30001-0001-0001-0001-000000000004" `
            -NewValue $InterfaceGuid
    }
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "Deterministic COM event test server for OxVba registered event lane parity." `
        -NewValue $Description
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "$ProjectName.csproj") `
        -OldValue "Deterministic COM event test server for OxVba registered event lane parity." `
        -NewValue $Description
    Replace-InFile `
        -Path (Join-Path $DestinationRoot "TestEventServer.cs") `
        -OldValue "return 42;" `
        -NewValue "return $PingValue;"
}

function New-AltTestEventServerProject {
    param(
        [string]$WorkspaceRoot,
        [string]$DestinationRoot
    )

    New-TestEventServerVariantProject `
        -WorkspaceRoot $WorkspaceRoot `
        -DestinationRoot $DestinationRoot `
        -ProjectName "OxVba.TestEventServerAlt" `
        -LibraryGuid "E2A30001-0001-0001-0001-000000000101" `
        -EventsGuid "E2A30001-0001-0001-0001-000000000102" `
        -ClassGuid "E2A30001-0001-0001-0001-000000000103" `
        -InterfaceGuid "E2A30001-0001-0001-0001-000000000104" `
        -PingValue 84 `
        -Description "Deterministic alt COM event test server for OxVba registered event lane parity."
}

function New-Alt2TestEventServerProject {
    param(
        [string]$WorkspaceRoot,
        [string]$DestinationRoot
    )

    New-TestEventServerVariantProject `
        -WorkspaceRoot $WorkspaceRoot `
        -DestinationRoot $DestinationRoot `
        -ProjectName "OxVba.TestEventServerAlt2" `
        -LibraryGuid "E2A30001-0001-0001-0001-000000000201" `
        -EventsGuid "E2A30001-0001-0001-0001-000000000202" `
        -ClassGuid "E2A30001-0001-0001-0001-000000000203" `
        -InterfaceGuid "E2A30001-0001-0001-0001-000000000204" `
        -PingValue 126 `
        -Description "Deterministic alt2 COM event test server for OxVba registered event lane parity."
}
