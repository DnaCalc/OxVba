Set-StrictMode -Version Latest

function New-ExcelOracleProcessStartInfo {
    param(
        [Parameter(Mandatory = $true)][string]$ExcelExecutable,
        [Parameter(Mandatory = $true)]$BootstrapWorkbook
    )
    if ([string]::IsNullOrWhiteSpace($ExcelExecutable) -or $null -eq $BootstrapWorkbook -or
        [string]::IsNullOrWhiteSpace([string]$BootstrapWorkbook.path)) {
        throw "excel-vba-oracle-bootstrap: Excel executable and bootstrap path are required"
    }
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $ExcelExecutable
    $startInfo.UseShellExecute = $false
    $startInfo.ArgumentList.Add("/x")
    $startInfo.ArgumentList.Add([string]$BootstrapWorkbook.path)
    if ($startInfo.ArgumentList.Count -ne 2 -or $startInfo.ArgumentList[0] -cne "/x" -or
        $startInfo.ArgumentList[1] -cne [string]$BootstrapWorkbook.path -or $startInfo.ArgumentList -contains "/n") {
        throw "excel-vba-oracle-bootstrap: invalid direct Excel launch argv"
    }
    return $startInfo
}

function New-ExcelOracleBootstrapWorkbook {
    param([Parameter(Mandatory = $true)][string]$Path)

    $parts = [ordered]@{
        "[Content_Types].xml" = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>'
        "_rels/.rels" = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>'
        "xl/workbook.xml" = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Bootstrap" sheetId="1" r:id="rId1"/></sheets></workbook>'
        "xl/_rels/workbook.xml.rels" = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>'
        "xl/worksheets/sheet1.xml" = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>'
    }

    $parent = Split-Path -Parent $Path
    if ($parent) { [void][IO.Directory]::CreateDirectory($parent) }
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    try {
        $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $true)
        try {
            $fixedTimestamp = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
            $utf8 = [Text.UTF8Encoding]::new($false)
            foreach ($part in $parts.GetEnumerator()) {
                $entry = $archive.CreateEntry([string]$part.Key, [IO.Compression.CompressionLevel]::NoCompression)
                $entry.LastWriteTime = $fixedTimestamp
                $entryStream = $entry.Open()
                try {
                    $bytes = $utf8.GetBytes([string]$part.Value)
                    $entryStream.Write($bytes, 0, $bytes.Length)
                }
                finally { $entryStream.Dispose() }
            }
        }
        finally { $archive.Dispose() }
    }
    finally { $stream.Dispose() }

    $resolved = (Resolve-Path -LiteralPath $Path).Path
    return [pscustomobject]@{
        schema = "oxvba.excel-vba-oracle-bootstrap-workbook.v1"
        path = $resolved
        sha256 = "sha256:$((Get-FileHash -LiteralPath $resolved -Algorithm SHA256).Hash.ToLowerInvariant())"
        package_parts = @($parts.Keys)
        macro_free = $true
    }
}

function Test-ExcelOracleBootstrapWorkbook {
    param([Parameter(Mandatory = $true)][AllowNull()]$Descriptor)

    if ($null -eq $Descriptor -or
        [string]$Descriptor.schema -ne "oxvba.excel-vba-oracle-bootstrap-workbook.v1" -or
        $Descriptor.macro_free -isnot [bool] -or -not [bool]$Descriptor.macro_free -or
        [string]::IsNullOrWhiteSpace([string]$Descriptor.path) -or
        [string]$Descriptor.sha256 -notmatch '^sha256:[0-9a-f]{64}$' -or
        -not (Test-Path -LiteralPath ([string]$Descriptor.path) -PathType Leaf) -or
        @($Descriptor.package_parts).Count -ne 5) {
        return $false
    }
    $actualHash = "sha256:$((Get-FileHash -LiteralPath ([string]$Descriptor.path) -Algorithm SHA256).Hash.ToLowerInvariant())"
    if ($actualHash -cne [string]$Descriptor.sha256) { return $false }

    $expectedParts = @("[Content_Types].xml", "_rels/.rels", "xl/workbook.xml", "xl/_rels/workbook.xml.rels", "xl/worksheets/sheet1.xml")
    if ((@($Descriptor.package_parts) -join "`n") -cne ($expectedParts -join "`n")) { return $false }
    try {
        $archive = [IO.Compression.ZipFile]::OpenRead([string]$Descriptor.path)
        try {
            $entryNames = @($archive.Entries | ForEach-Object FullName)
            if (($entryNames -join "`n") -cne ($expectedParts -join "`n") -or
                @($entryNames | Where-Object { $_ -match '(?i)vbaProject|macrosheet|xl4' }).Count -gt 0) {
                return $false
            }
            $documents = @{}
            foreach ($entry in $archive.Entries) {
                $reader = [IO.StreamReader]::new($entry.Open(), [Text.UTF8Encoding]::new($false))
                try { $documents[$entry.FullName] = [xml]$reader.ReadToEnd() }
                finally { $reader.Dispose() }
                if ($null -eq $documents[$entry.FullName].DocumentElement) { return $false }
            }

            $contentTypeParts = @($documents["[Content_Types].xml"].Types.Override | ForEach-Object { ([string]$_.PartName).TrimStart('/') })
            if (($contentTypeParts -join "`n") -cne (@("xl/workbook.xml", "xl/worksheets/sheet1.xml") -join "`n") -or
                @($documents["[Content_Types].xml"].Types.Override | Where-Object { [string]$_.ContentType -match '(?i)macroEnabled' }).Count -gt 0) {
                return $false
            }

            $rootRelationship = @($documents["_rels/.rels"].Relationships.Relationship)
            $workbookRelationships = @($documents["xl/_rels/workbook.xml.rels"].Relationships.Relationship)
            if ($rootRelationship.Count -ne 1 -or [string]$rootRelationship[0].Id -cne "rId1" -or
                [string]$rootRelationship[0].Type -cne "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" -or
                [string]$rootRelationship[0].Target -cne "xl/workbook.xml" -or
                $workbookRelationships.Count -ne 1 -or [string]$workbookRelationships[0].Id -cne "rId1" -or
                [string]$workbookRelationships[0].Type -cne "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" -or
                [string]$workbookRelationships[0].Target -cne "worksheets/sheet1.xml") {
                return $false
            }
            $sheet = @($documents["xl/workbook.xml"].workbook.sheets.sheet)
            if ($sheet.Count -ne 1 -or [string]$sheet[0].name -cne "Bootstrap" -or
                [string]$sheet[0].sheetId -cne "1" -or
                [string]$sheet[0].GetAttribute("id", "http://schemas.openxmlformats.org/officeDocument/2006/relationships") -cne "rId1") {
                return $false
            }
            return $entryNames -contains [string]$rootRelationship[0].Target -and
                $entryNames -contains ("xl/" + [string]$workbookRelationships[0].Target)
        }
        finally { $archive.Dispose() }
    }
    catch { return $false }
}
