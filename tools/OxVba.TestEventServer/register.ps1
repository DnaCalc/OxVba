param(
    [switch]$Unregister,
    [string]$Configuration = "Debug",
    [ValidateSet("CurrentUser", "Machine")]
    [string]$Scope = "CurrentUser",
    [switch]$ExportTypeLibOnly,
    [switch]$SkipTypeLibExport,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false

function Resolve-RegAsm {
    $regasmCandidates = @(
        "${env:WINDIR}\Microsoft.NET\Framework64\v4.0.30319\RegAsm.exe",
        "${env:WINDIR}\Microsoft.NET\Framework\v4.0.30319\RegAsm.exe"
    )
    $regasm = $regasmCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $regasm) {
        throw "RegAsm.exe not found in .NET Framework directories"
    }
    $regasm
}

function Resolve-TlbExp {
    $tlbExpCandidates = @(
        "C:\Program Files (x86)\Microsoft SDKs\Windows\v10.0A\bin\NETFX 4.8.1 Tools\x64\TlbExp.exe",
        "C:\Program Files (x86)\Microsoft SDKs\Windows\v10.0A\bin\NETFX 4.8.1 Tools\TlbExp.exe",
        "C:\Program Files (x86)\Microsoft SDKs\Windows\v10.0A\bin\NETFX 4.8 Tools\x64\TlbExp.exe",
        "C:\Program Files (x86)\Microsoft SDKs\Windows\v10.0A\bin\NETFX 4.8 Tools\TlbExp.exe"
    )
    $tlbexp = $tlbExpCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $tlbexp) {
        throw "TlbExp.exe not found in the expected Windows SDK directories"
    }
    $tlbexp
}

function Export-TypeLib {
    param(
        [string]$AssemblyPath,
        [string]$TypeLibPath
    )

    $tlbexp = Resolve-TlbExp
    Write-Host "[oxvba] Exporting type library..."
    & $tlbexp $AssemblyPath /out:$TypeLibPath
    if ($LASTEXITCODE -ne 0) {
        throw "TlbExp failed (exit=$LASTEXITCODE)"
    }
    if (-not (Test-Path $TypeLibPath)) {
        throw "Type library not found at $TypeLibPath after TlbExp"
    }
}

function Register-CurrentUserServer {
    param(
        [string]$TemplatePath,
        [string]$AssemblyPath
    )

    $template = Get-Content $TemplatePath -Raw
    $codeBaseUri = ([System.Uri]::new($AssemblyPath)).AbsoluteUri
    $rewritten = [System.Text.RegularExpressions.Regex]::Replace(
        $template,
        '("CodeBase"=")file:///[^"\r\n]+(")',
        { param($match) $match.Groups[1].Value + $codeBaseUri + $match.Groups[2].Value }
    )
    $tempRegPath = Join-Path $env:TEMP ("OxVba.TestEventServer.{0}.reg" -f ([System.Guid]::NewGuid().ToString("N")))
    try {
        Set-Content -Path $tempRegPath -Value $rewritten -Encoding ASCII
        & reg import $tempRegPath
        if ($LASTEXITCODE -ne 0) {
            throw "reg import failed (exit=$LASTEXITCODE)"
        }
    } finally {
        Remove-Item -Path $tempRegPath -Force -ErrorAction SilentlyContinue
    }
}

function Unregister-CurrentUserServer {
    $paths = @(
        "HKCU\Software\Classes\OxVba.TestEventServer",
        "HKCU\Software\Classes\CLSID\{E2A30001-0001-0001-0001-000000000004}"
    )
    foreach ($path in $paths) {
        & reg delete $path /f 2>$null
    }
}

Push-Location $PSScriptRoot
try {
    $assemblyPath = Join-Path (Join-Path (Join-Path (Join-Path $PSScriptRoot "bin") $Configuration) "net48") "OxVba.TestEventServer.dll"
    $typeLibPath = Join-Path (Split-Path $assemblyPath -Parent) "OxVba.TestEventServer.tlb"
    $currentUserTemplatePath = Join-Path $PSScriptRoot "OxVba.TestEventServer.hkcu.reg"

    if (-not $SkipBuild) {
        Write-Host "[oxvba] Building OxVba.TestEventServer ($Configuration)..."
        dotnet build -c $Configuration
        if ($LASTEXITCODE -ne 0) {
            throw "dotnet build failed (exit=$LASTEXITCODE)"
        }
    }

    if (-not (Test-Path $assemblyPath)) {
        throw "Assembly not found at $assemblyPath after build"
    }

    if (-not $SkipTypeLibExport -or $ExportTypeLibOnly) {
        Export-TypeLib -AssemblyPath $assemblyPath -TypeLibPath $typeLibPath
    }

    if ($ExportTypeLibOnly) {
        Write-Host "[oxvba] Type library exported successfully."
        Write-Host "[oxvba] TypeLib: $typeLibPath"
        return
    }

    if ($Unregister) {
        Write-Host "[oxvba] Unregistering OxVba.TestEventServer ($Scope)..."
        if ($Scope -eq "CurrentUser") {
            Unregister-CurrentUserServer
        } else {
            $regasm = Resolve-RegAsm
            & $regasm /unregister /codebase $assemblyPath
            if ($LASTEXITCODE -ne 0) {
                throw "RegAsm /unregister failed (exit=$LASTEXITCODE)"
            }
        }
        Write-Host "[oxvba] OxVba.TestEventServer unregistered."
    } else {
        Write-Host "[oxvba] Registering OxVba.TestEventServer ($Scope)..."
        if ($Scope -eq "CurrentUser") {
            Register-CurrentUserServer -TemplatePath $currentUserTemplatePath -AssemblyPath $assemblyPath
        } else {
            $regasm = Resolve-RegAsm
            & $regasm /codebase $assemblyPath
            if ($LASTEXITCODE -ne 0) {
                throw "RegAsm /codebase failed (exit=$LASTEXITCODE)"
            }
        }
        Write-Host "[oxvba] OxVba.TestEventServer registered successfully."
        Write-Host "[oxvba] ProgID: OxVba.TestEventServer"
        Write-Host "[oxvba] Assembly: $assemblyPath"
        if (Test-Path $typeLibPath) {
            Write-Host "[oxvba] TypeLib: $typeLibPath"
        }
    }
} finally {
    Pop-Location
}
