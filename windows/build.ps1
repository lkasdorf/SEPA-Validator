<#
.SYNOPSIS
    Baut SEPA-Validator.exe mit eingebetteten XSD-Schemas.
.DESCRIPTION
    1. Liest alle XSD-Dateien aus xml_schema/
    2. Komprimiert sie (GZip) und kodiert als Base64
    3. Bettet sie in SEPA-Validator.ps1 ein
    4. Kompiliert mit ps2exe zu einer eigenstaendigen .exe
.NOTES
    Voraussetzung: Install-Module ps2exe -Scope CurrentUser
#>
param(
    [string]$SchemaDir = (Join-Path $PSScriptRoot '..\xml_schema'),
    [string]$OutDir    = (Join-Path $PSScriptRoot 'dist')
)

$ErrorActionPreference = 'Stop'

# --- ps2exe pruefen ---
if (-not (Get-Command Invoke-PS2EXE -ErrorAction SilentlyContinue)) {
    Write-Host 'ps2exe nicht gefunden. Installiere...' -ForegroundColor Yellow
    Install-Module ps2exe -Scope CurrentUser -Force
}

# --- Schemas komprimieren ---
Write-Host 'Komprimiere Schemas...' -ForegroundColor Cyan
$schemaEntries = [System.Collections.Generic.List[string]]::new()

foreach ($xsd in (Get-ChildItem -Path $SchemaDir -Filter '*.xsd')) {
    $content = [System.IO.File]::ReadAllBytes($xsd.FullName)
    $ms = New-Object System.IO.MemoryStream
    $gs = New-Object System.IO.Compression.GZipStream($ms, [System.IO.Compression.CompressionMode]::Compress)
    $gs.Write($content, 0, $content.Length)
    $gs.Close()
    $b64 = [System.Convert]::ToBase64String($ms.ToArray())
    $ms.Close()

    $schemaEntries.Add("    '$($xsd.Name)' = '$b64'")
    $originalKB = [math]::Round($xsd.Length / 1024, 1)
    $compressedKB = [math]::Round($b64.Length * 3 / 4 / 1024, 1)
    Write-Host "  $($xsd.Name): ${originalKB}KB -> ${compressedKB}KB" -ForegroundColor DarkGray
}

$embeddedBlock = "`$EmbeddedSchemas = @{`n$($schemaEntries -join "`n")`n}"

# --- PS1 mit Schemas erzeugen ---
Write-Host 'Erzeuge Build-Version...' -ForegroundColor Cyan
$sourcePs1 = Join-Path $PSScriptRoot 'SEPA-Validator.ps1'
$ps1Content = Get-Content $sourcePs1 -Raw

if ($ps1Content -notmatch '# @@EMBEDDED_SCHEMAS@@') {
    Write-Error 'Marker "# @@EMBEDDED_SCHEMAS@@" nicht in SEPA-Validator.ps1 gefunden.'
    exit 1
}

$buildPs1Content = $ps1Content -replace '# @@EMBEDDED_SCHEMAS@@', $embeddedBlock

if (-not (Test-Path $OutDir)) { [void](New-Item -ItemType Directory -Path $OutDir) }
$buildPs1 = Join-Path $OutDir 'SEPA-Validator-build.ps1'
[System.IO.File]::WriteAllText($buildPs1, $buildPs1Content, [System.Text.Encoding]::UTF8)

# --- EXE kompilieren ---
Write-Host 'Kompiliere EXE...' -ForegroundColor Cyan
$exePath = Join-Path $OutDir 'SEPA-Validator.exe'

Invoke-PS2EXE -InputFile $buildPs1 `
    -OutputFile $exePath `
    -NoConsole `
    -STA `
    -Title 'SEPA XML Validator' `
    -Description 'SEPA XML Schema Validator' `
    -Company 'SEPA-Validator' `
    -Version '1.0.0.0' `
    -Copyright "(c) $(Get-Date -Format yyyy)" `
    -RequireAdmin:$false

# --- Aufraeumen ---
Remove-Item $buildPs1 -Force

$exeSize = [math]::Round((Get-Item $exePath).Length / 1024 / 1024, 2)
Write-Host "`nFertig: $exePath ($exeSize MB)" -ForegroundColor Green
Write-Host 'Die EXE enthaelt alle Schemas und ist ohne weitere Dateien lauffaehig.' -ForegroundColor Green
