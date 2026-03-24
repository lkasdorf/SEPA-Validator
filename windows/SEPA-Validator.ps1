#Requires -Version 5.1
<#
.SYNOPSIS
    SEPA XML Validator - Windows GUI Tool
.DESCRIPTION
    Validiert SEPA-XML-Dateien gegen ISO 20022 XSD-Schemas.
    Unterstuetzt Drag & Drop, Dateiauswahl und Ordner-Validierung.
.NOTES
    Voraussetzung: XSD-Schemas im Unterordner "schemas\" neben diesem Script.
#>

try {

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# --- Schema-Konfiguration ---

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$SchemaDir = Join-Path $ScriptDir 'schemas'

# Namespace -> XSD-Datei (Reihenfolge: spezifischere GBIC-Varianten zuerst)
$SchemaMap = [ordered]@{
    'urn:iso:std:iso:20022:tech:xsd:pain.001.001.03' = 'pain.001.001.03.xsd'
    'urn:iso:std:iso:20022:tech:xsd:pain.001.001.09' = 'pain.001.001.09.xsd'
    'urn:iso:std:iso:20022:tech:xsd:pain.002.001.10' = 'pain.002.001.10.xsd'
    'urn:iso:std:iso:20022:tech:xsd:pain.007.001.09' = 'pain.007.001.09.xsd'
    'urn:iso:std:iso:20022:tech:xsd:pain.008.001.02' = 'pain.008.001.02.xsd'
    'urn:iso:std:iso:20022:tech:xsd:pain.008.001.08' = 'pain.008.001.08.xsd'
    'urn:iso:std:iso:20022:tech:xsd:camt.054.001.08' = 'camt.054.001.08.xsd'
    'urn:conxml:xsd:container.nnn.001.GBIC4'         = 'container.nnn.001.GBIC4.xsd'
}

# Vorkompilierter Schema-Cache (wird einmal pro Namespace geladen)
$script:SchemaCache = @{}

# --- Validierungslogik ---

function Get-XmlNamespace {
    param([string]$FilePath)
    try {
        $settings = New-Object System.Xml.XmlReaderSettings
        $settings.XmlResolver = $null
        $settings.DtdProcessing = [System.Xml.DtdProcessing]::Ignore
        $reader = [System.Xml.XmlReader]::Create($FilePath, $settings)
        while ($reader.Read()) {
            if ($reader.NodeType -eq [System.Xml.XmlNodeType]::Element) {
                $ns = $reader.NamespaceURI
                $reader.Close()
                return $ns
            }
        }
        $reader.Close()
    } catch {
        return $null
    }
    return $null
}

function Test-SepaXml {
    param([string]$FilePath)

    $result = [PSCustomObject]@{
        Datei     = [System.IO.Path]::GetFileName($FilePath)
        Pfad      = $FilePath
        Namespace = ''
        Schema    = ''
        Status    = ''
        Meldungen = [System.Collections.Generic.List[PSCustomObject]]::new()
    }

    # XML lesbar?
    if (-not (Test-Path $FilePath)) {
        $result.Status = 'FEHLER'
        $result.Meldungen.Add([PSCustomObject]@{ Typ = 'Fehler'; Text = 'Datei nicht gefunden.' })
        return $result
    }

    # Namespace ermitteln
    $ns = Get-XmlNamespace -FilePath $FilePath
    if (-not $ns) {
        $result.Status = 'FEHLER'
        $result.Meldungen.Add([PSCustomObject]@{ Typ = 'Fehler'; Text = 'Kein XML-Namespace erkannt. Datei ist moeglicherweise kein gueltiges XML.' })
        return $result
    }
    $result.Namespace = $ns

    # Schema zuordnen
    $schemaFile = $null
    if ($SchemaMap.Contains($ns)) {
        $schemaFile = Join-Path $SchemaDir $SchemaMap[$ns]
    }

    if (-not $schemaFile -or -not (Test-Path $schemaFile)) {
        $result.Status = 'KEIN SCHEMA'
        $friendlyNs = $ns
        $result.Meldungen.Add([PSCustomObject]@{
            Typ  = 'Warnung'
            Text = "Kein passendes Schema fuer Namespace: $friendlyNs"
        })
        return $result
    }
    $result.Schema = [System.IO.Path]::GetFileName($schemaFile)

    # Schema aus Cache laden oder einmalig kompilieren
    if (-not $script:SchemaCache.ContainsKey($ns)) {
        $newSet = New-Object System.Xml.Schema.XmlSchemaSet
        $newSet.XmlResolver = $null  # Keine Netzwerkzugriffe
        try {
            $schemaReader = [System.Xml.XmlReader]::Create(
                $schemaFile,
                (New-Object System.Xml.XmlReaderSettings -Property @{ XmlResolver = $null })
            )
            [void]$newSet.Add($ns, $schemaReader)
            $schemaReader.Close()
            $newSet.Compile()
            $script:SchemaCache[$ns] = $newSet
        } catch {
            $result.Status = 'FEHLER'
            $result.Meldungen.Add([PSCustomObject]@{ Typ = 'Fehler'; Text = "Schema konnte nicht geladen werden: $_" })
            return $result
        }
    }
    $schemaSet = $script:SchemaCache[$ns]

    $readerSettings = New-Object System.Xml.XmlReaderSettings
    $readerSettings.ValidationType = [System.Xml.ValidationType]::Schema
    $readerSettings.Schemas = $schemaSet
    $readerSettings.XmlResolver = $null  # Keine Netzwerkzugriffe
    $readerSettings.ValidationFlags =
        [System.Xml.Schema.XmlSchemaValidationFlags]::ReportValidationWarnings

    $messages = $result.Meldungen
    $readerSettings.add_ValidationEventHandler([System.Xml.Schema.ValidationEventHandler]{
        param($sender, [System.Xml.Schema.ValidationEventArgs]$e)
        $typ = if ($e.Severity -eq [System.Xml.Schema.XmlSeverityType]::Error) { 'Fehler' } else { 'Warnung' }
        $zeile = ''
        if ($e.Exception -and $e.Exception.LineNumber -gt 0) {
            $zeile = " (Zeile $($e.Exception.LineNumber), Spalte $($e.Exception.LinePosition))"
        }
        $messages.Add([PSCustomObject]@{
            Typ  = $typ
            Text = "$($e.Message)$zeile"
        })
    })

    try {
        $reader = [System.Xml.XmlReader]::Create($FilePath, $readerSettings)
        while ($reader.Read()) { }
        $reader.Close()
    } catch {
        $result.Meldungen.Add([PSCustomObject]@{ Typ = 'Fehler'; Text = "XML-Lesefehler: $_" })
    }

    $fehler = @($result.Meldungen | Where-Object { $_.Typ -eq 'Fehler' }).Count
    $warnungen = @($result.Meldungen | Where-Object { $_.Typ -eq 'Warnung' }).Count

    if ($fehler -gt 0) {
        $result.Status = "FEHLERHAFT ($fehler Fehler, $warnungen Warnungen)"
    } elseif ($warnungen -gt 0) {
        $result.Status = "WARNUNGEN ($warnungen)"
    } else {
        $result.Status = 'OK'
    }

    return $result
}

# --- GUI ---

$form = New-Object System.Windows.Forms.Form
$form.Text = 'SEPA XML Validator'
$form.Size = New-Object System.Drawing.Size(960, 700)
$form.MinimumSize = New-Object System.Drawing.Size(700, 500)
$form.StartPosition = 'CenterScreen'
$form.Font = New-Object System.Drawing.Font('Segoe UI', 9)
$form.AllowDrop = $true
$form.BackColor = [System.Drawing.Color]::FromArgb(245, 245, 245)

$iconBase64 = $null  # Platzhalter fuer optionales Icon

# --- Header-Panel ---
$headerPanel = New-Object System.Windows.Forms.Panel
$headerPanel.Dock = 'Top'
$headerPanel.Height = 56
$headerPanel.BackColor = [System.Drawing.Color]::FromArgb(0, 90, 158)

$titleLabel = New-Object System.Windows.Forms.Label
$titleLabel.Text = 'SEPA XML Validator'
$titleLabel.ForeColor = [System.Drawing.Color]::White
$titleLabel.Font = New-Object System.Drawing.Font('Segoe UI', 16, [System.Drawing.FontStyle]::Bold)
$titleLabel.AutoSize = $true
$titleLabel.Location = New-Object System.Drawing.Point(16, 12)
$headerPanel.Controls.Add($titleLabel)

$versionLabel = New-Object System.Windows.Forms.Label
$versionLabel.Text = 'v1.0'
$versionLabel.ForeColor = [System.Drawing.Color]::FromArgb(180, 210, 240)
$versionLabel.Font = New-Object System.Drawing.Font('Segoe UI', 9)
$versionLabel.AutoSize = $true
$versionLabel.Location = New-Object System.Drawing.Point(250, 22)
$headerPanel.Controls.Add($versionLabel)

$form.Controls.Add($headerPanel)

# --- Toolbar ---
$toolPanel = New-Object System.Windows.Forms.Panel
$toolPanel.Dock = 'Top'
$toolPanel.Height = 48
$toolPanel.Padding = New-Object System.Windows.Forms.Padding(12, 8, 12, 8)

$btnDatei = New-Object System.Windows.Forms.Button
$btnDatei.Text = 'Dateien waehlen...'
$btnDatei.Size = New-Object System.Drawing.Size(140, 32)
$btnDatei.Location = New-Object System.Drawing.Point(12, 8)
$btnDatei.FlatStyle = 'Flat'
$btnDatei.BackColor = [System.Drawing.Color]::FromArgb(0, 120, 212)
$btnDatei.ForeColor = [System.Drawing.Color]::White
$btnDatei.FlatAppearance.BorderSize = 0
$btnDatei.Cursor = [System.Windows.Forms.Cursors]::Hand
$toolPanel.Controls.Add($btnDatei)

$btnOrdner = New-Object System.Windows.Forms.Button
$btnOrdner.Text = 'Ordner waehlen...'
$btnOrdner.Size = New-Object System.Drawing.Size(140, 32)
$btnOrdner.Location = New-Object System.Drawing.Point(162, 8)
$btnOrdner.FlatStyle = 'Flat'
$btnOrdner.BackColor = [System.Drawing.Color]::FromArgb(0, 120, 212)
$btnOrdner.ForeColor = [System.Drawing.Color]::White
$btnOrdner.FlatAppearance.BorderSize = 0
$btnOrdner.Cursor = [System.Windows.Forms.Cursors]::Hand
$toolPanel.Controls.Add($btnOrdner)

$btnExport = New-Object System.Windows.Forms.Button
$btnExport.Text = 'Ergebnis exportieren...'
$btnExport.Size = New-Object System.Drawing.Size(160, 32)
$btnExport.Location = New-Object System.Drawing.Point(312, 8)
$btnExport.FlatStyle = 'Flat'
$btnExport.BackColor = [System.Drawing.Color]::FromArgb(100, 100, 100)
$btnExport.ForeColor = [System.Drawing.Color]::White
$btnExport.FlatAppearance.BorderSize = 0
$btnExport.Cursor = [System.Windows.Forms.Cursors]::Hand
$btnExport.Enabled = $false
$toolPanel.Controls.Add($btnExport)

$form.Controls.Add($toolPanel)

# --- Drop-Zone ---
$dropPanel = New-Object System.Windows.Forms.Panel
$dropPanel.Dock = 'Top'
$dropPanel.Height = 64
$dropPanel.Padding = New-Object System.Windows.Forms.Padding(12, 0, 12, 8)

$dropLabel = New-Object System.Windows.Forms.Label
$dropLabel.Text = 'XML-Dateien hierher ziehen oder oben auswaehlen'
$dropLabel.Dock = 'Fill'
$dropLabel.TextAlign = 'MiddleCenter'
$dropLabel.Font = New-Object System.Drawing.Font('Segoe UI', 10)
$dropLabel.ForeColor = [System.Drawing.Color]::FromArgb(100, 100, 100)
$dropLabel.BackColor = [System.Drawing.Color]::White
$dropLabel.BorderStyle = 'FixedSingle'
$dropLabel.AllowDrop = $true
$dropPanel.Controls.Add($dropLabel)
$form.Controls.Add($dropPanel)

# --- Fortschrittsanzeige ---
$progressPanel = New-Object System.Windows.Forms.Panel
$progressPanel.Dock = 'Top'
$progressPanel.Height = 24
$progressPanel.Padding = New-Object System.Windows.Forms.Padding(12, 4, 12, 4)
$progressPanel.Visible = $false

$progressBar = New-Object System.Windows.Forms.ProgressBar
$progressBar.Dock = 'Fill'
$progressBar.Style = 'Continuous'
$progressBar.Minimum = 0
$progressBar.Maximum = 100
$form.Controls.Add($progressPanel)
$progressPanel.Controls.Add($progressBar)

# --- Dateiliste (obere Haelfte) ---
$splitContainer = New-Object System.Windows.Forms.SplitContainer
$splitContainer.Dock = 'Fill'
$splitContainer.Orientation = 'Horizontal'
$splitContainer.SplitterDistance = 220
$splitContainer.SplitterWidth = 6
$splitContainer.BackColor = [System.Drawing.Color]::FromArgb(220, 220, 220)

# Datei-Grid
$grid = New-Object System.Windows.Forms.DataGridView
$grid.Dock = 'Fill'
$grid.ReadOnly = $true
$grid.AllowUserToAddRows = $false
$grid.AllowUserToDeleteRows = $false
$grid.AllowUserToResizeRows = $false
$grid.SelectionMode = 'FullRowSelect'
$grid.MultiSelect = $false
$grid.RowHeadersVisible = $false
$grid.AutoSizeColumnsMode = 'Fill'
$grid.BackgroundColor = [System.Drawing.Color]::White
$grid.BorderStyle = 'None'
$grid.CellBorderStyle = 'SingleHorizontal'
$grid.DefaultCellStyle.SelectionBackColor = [System.Drawing.Color]::FromArgb(0, 120, 212)
$grid.ColumnHeadersDefaultCellStyle.BackColor = [System.Drawing.Color]::FromArgb(240, 240, 240)
$grid.ColumnHeadersDefaultCellStyle.Font = New-Object System.Drawing.Font('Segoe UI', 9, [System.Drawing.FontStyle]::Bold)
$grid.ColumnHeadersBorderStyle = 'Single'
$grid.EnableHeadersVisualStyles = $false

[void]$grid.Columns.Add('Datei', 'Datei')
[void]$grid.Columns.Add('Namespace', 'Namespace')
[void]$grid.Columns.Add('Schema', 'Schema')
[void]$grid.Columns.Add('Status', 'Status')

$grid.Columns['Datei'].FillWeight = 25
$grid.Columns['Namespace'].FillWeight = 35
$grid.Columns['Schema'].FillWeight = 20
$grid.Columns['Status'].FillWeight = 20
$grid.AllowDrop = $true

$splitContainer.Panel1.Controls.Add($grid)

# Detail-Ansicht
$detailBox = New-Object System.Windows.Forms.RichTextBox
$detailBox.Dock = 'Fill'
$detailBox.ReadOnly = $true
$detailBox.Font = New-Object System.Drawing.Font('Consolas', 9.5)
$detailBox.BackColor = [System.Drawing.Color]::FromArgb(30, 30, 30)
$detailBox.ForeColor = [System.Drawing.Color]::FromArgb(220, 220, 220)
$detailBox.BorderStyle = 'None'
$detailBox.Text = 'Details werden hier angezeigt, wenn eine Datei ausgewaehlt wird.'

$splitContainer.Panel2.Controls.Add($detailBox)
$form.Controls.Add($splitContainer)

# --- Statusleiste ---
$statusStrip = New-Object System.Windows.Forms.StatusStrip
$statusLabel = New-Object System.Windows.Forms.ToolStripStatusLabel
$statusLabel.Text = 'Bereit'
$statusLabel.Spring = $true
$statusLabel.TextAlign = 'MiddleLeft'
[void]$statusStrip.Items.Add($statusLabel)
$form.Controls.Add($statusStrip)

# --- Daten-Speicher ---
$script:Results = [System.Collections.Generic.List[PSCustomObject]]::new()

# --- Hilfsfunktionen ---

function Update-StatusBar {
    param([string]$Text)
    $statusLabel.Text = $Text
    $form.Refresh()
}

function Add-ResultToGrid {
    param([PSCustomObject]$Result)

    $rowIndex = $grid.Rows.Add($Result.Datei, $Result.Namespace, $Result.Schema, $Result.Status)
    $row = $grid.Rows[$rowIndex]

    switch -Wildcard ($Result.Status) {
        'OK'           { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(16, 124, 16) }
        'FEHLERHAFT*'  { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(196, 43, 28) }
        'WARNUNGEN*'   { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(157, 93, 0) }
        'KEIN SCHEMA'  { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(157, 93, 0) }
        'FEHLER'       { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(196, 43, 28) }
    }
}

function Show-Detail {
    param([int]$Index)

    if ($Index -lt 0 -or $Index -ge $script:Results.Count) { return }
    $r = $script:Results[$Index]

    $detailBox.Clear()

    # Dateiname
    $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(86, 156, 214)
    $detailBox.AppendText("Datei: $($r.Pfad)`n")

    $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(220, 220, 220)
    $detailBox.AppendText("Namespace: $($r.Namespace)`n")
    $detailBox.AppendText("Schema: $($r.Schema)`n")

    # Status
    $statusColor = switch -Wildcard ($r.Status) {
        'OK'           { [System.Drawing.Color]::FromArgb(78, 201, 176) }
        'FEHLERHAFT*'  { [System.Drawing.Color]::FromArgb(244, 71, 71) }
        'WARNUNGEN*'   { [System.Drawing.Color]::FromArgb(255, 200, 50) }
        default        { [System.Drawing.Color]::FromArgb(255, 200, 50) }
    }
    $detailBox.SelectionColor = $statusColor
    $detailBox.AppendText("Status: $($r.Status)`n")

    $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(220, 220, 220)
    $detailBox.AppendText("`n")

    if ($r.Meldungen.Count -eq 0) {
        $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(78, 201, 176)
        $detailBox.AppendText("Keine Fehler oder Warnungen.`n")
    } else {
        $nr = 1
        foreach ($m in $r.Meldungen) {
            if ($m.Typ -eq 'Fehler') {
                $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(244, 71, 71)
                $detailBox.AppendText("[$nr] FEHLER: ")
            } else {
                $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(255, 200, 50)
                $detailBox.AppendText("[$nr] WARNUNG: ")
            }
            $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(220, 220, 220)
            $detailBox.AppendText("$($m.Text)`n`n")
            $nr++
        }
    }

    $detailBox.SelectionStart = 0
    $detailBox.ScrollToCaret()
}

function Start-Validation {
    param([string[]]$Files)

    $xmlFiles = $Files | Where-Object { $_ -match '\.xml$' -and $_ -notmatch ':Zone\.Identifier$' }

    if (-not $xmlFiles) {
        [System.Windows.Forms.MessageBox]::Show(
            'Keine XML-Dateien gefunden.',
            'Hinweis',
            'OK',
            'Information'
        )
        return
    }

    $grid.Rows.Clear()
    $script:Results.Clear()
    $detailBox.Clear()

    $total = @($xmlFiles).Count
    $ok = 0; $fail = 0; $warn = 0; $noSchema = 0; $nr = 0

    $progressBar.Value = 0
    $progressBar.Maximum = $total
    $progressPanel.Visible = $true
    $form.Cursor = [System.Windows.Forms.Cursors]::WaitCursor

    foreach ($file in $xmlFiles) {
        $nr++
        $fileName = Split-Path -Leaf $file
        Update-StatusBar "Validiere $nr / $total : $fileName"
        $progressBar.Value = $nr
        [System.Windows.Forms.Application]::DoEvents()

        try {
            $result = Test-SepaXml -FilePath $file
        } catch {
            $result = [PSCustomObject]@{
                Datei     = $fileName
                Pfad      = $file
                Namespace = ''
                Schema    = ''
                Status    = 'FEHLER'
                Meldungen = [System.Collections.Generic.List[PSCustomObject]]::new()
            }
            $result.Meldungen.Add([PSCustomObject]@{ Typ = 'Fehler'; Text = "Unerwarteter Fehler: $_" })
        }
        $script:Results.Add($result)
        Add-ResultToGrid -Result $result

        switch -Wildcard ($result.Status) {
            'OK'           { $ok++ }
            'FEHLERHAFT*'  { $fail++ }
            'WARNUNGEN*'   { $warn++ }
            'KEIN SCHEMA'  { $noSchema++ }
            'FEHLER'       { $fail++ }
        }
    }

    $progressPanel.Visible = $false
    $form.Cursor = [System.Windows.Forms.Cursors]::Default
    Update-StatusBar "$total Dateien | OK: $ok | Fehlerhaft: $fail | Warnungen: $warn | Kein Schema: $noSchema"
    $btnExport.Enabled = ($script:Results.Count -gt 0)

    if ($grid.Rows.Count -gt 0) {
        $grid.Rows[0].Selected = $true
        Show-Detail -Index 0
    }
}

# --- Event-Handler ---

$grid.add_SelectionChanged({
    if ($grid.SelectedRows.Count -gt 0) {
        Show-Detail -Index $grid.SelectedRows[0].Index
    }
})

$script:HandleDragEnter = {
    if ($_.Data.GetDataPresent([System.Windows.Forms.DataFormats]::FileDrop)) {
        $_.Effect = [System.Windows.Forms.DragDropEffects]::Copy
        $dropLabel.BackColor = [System.Drawing.Color]::FromArgb(230, 243, 255)
        $dropLabel.Text = 'Loslassen zum Validieren...'
    }
}

$script:HandleDragLeave = {
    $dropLabel.BackColor = [System.Drawing.Color]::White
    $dropLabel.Text = 'XML-Dateien hierher ziehen oder oben auswaehlen'
}

$script:HandleDragDrop = {
    $dropLabel.BackColor = [System.Drawing.Color]::White
    $dropLabel.Text = 'XML-Dateien hierher ziehen oder oben auswaehlen'

    $dropped = $_.Data.GetData([System.Windows.Forms.DataFormats]::FileDrop)
    $allFiles = [System.Collections.Generic.List[string]]::new()

    foreach ($item in $dropped) {
        if (Test-Path $item -PathType Container) {
            Get-ChildItem -Path $item -Filter '*.xml' -Recurse -File | ForEach-Object { $allFiles.Add($_.FullName) }
        } else {
            $allFiles.Add($item)
        }
    }

    Start-Validation -Files $allFiles.ToArray()
}

# Drag & Drop auf alle relevanten Controls
foreach ($ctrl in @($form, $dropLabel, $grid)) {
    $ctrl.add_DragEnter($script:HandleDragEnter)
    $ctrl.add_DragLeave($script:HandleDragLeave)
    $ctrl.add_DragDrop($script:HandleDragDrop)
}

$btnDatei.add_Click({
    $dlg = New-Object System.Windows.Forms.OpenFileDialog
    $dlg.Title = 'SEPA-XML-Dateien auswaehlen'
    $dlg.Filter = 'XML-Dateien (*.xml)|*.xml|Alle Dateien (*.*)|*.*'
    $dlg.Multiselect = $true
    if ($dlg.ShowDialog() -eq 'OK') {
        Start-Validation -Files $dlg.FileNames
    }
})

$btnOrdner.add_Click({
    $dlg = New-Object System.Windows.Forms.FolderBrowserDialog
    $dlg.Description = 'Ordner mit SEPA-XML-Dateien auswaehlen'
    $dlg.ShowNewFolderButton = $false
    if ($dlg.ShowDialog() -eq 'OK') {
        $files = Get-ChildItem -Path $dlg.SelectedPath -Filter '*.xml' -Recurse -File | ForEach-Object { $_.FullName }
        Start-Validation -Files $files
    }
})

$btnExport.add_Click({
    $dlg = New-Object System.Windows.Forms.SaveFileDialog
    $dlg.Title = 'Validierungsergebnis exportieren'
    $dlg.Filter = 'CSV-Datei (*.csv)|*.csv|Textdatei (*.txt)|*.txt'
    $dlg.FileName = "SEPA_Validierung_$(Get-Date -Format 'yyyyMMdd_HHmmss')"
    if ($dlg.ShowDialog() -eq 'OK') {
        $lines = [System.Collections.Generic.List[string]]::new()
        $lines.Add('Datei;Namespace;Schema;Status;Fehler/Warnungen')
        foreach ($r in $script:Results) {
            $msgs = ($r.Meldungen | ForEach-Object { "$($_.Typ): $($_.Text)" }) -join ' | '
            $lines.Add("$($r.Datei);$($r.Namespace);$($r.Schema);$($r.Status);$msgs")
        }
        [System.IO.File]::WriteAllLines($dlg.FileName, $lines, [System.Text.Encoding]::UTF8)
        Update-StatusBar "Export gespeichert: $($dlg.FileName)"
    }
})

# --- Schema-Verzeichnis pruefen ---
if (-not (Test-Path $SchemaDir)) {
    [System.Windows.Forms.MessageBox]::Show(
        "Schema-Ordner nicht gefunden:`n$SchemaDir`n`nBitte legen Sie einen Ordner 'schemas' neben diesem Script an und kopieren Sie die XSD-Dateien hinein.",
        'Schema-Ordner fehlt',
        'OK',
        'Warning'
    )
}

# --- Start ---
[void]$form.ShowDialog()

} catch {
    [System.Windows.Forms.MessageBox]::Show(
        "Unerwarteter Fehler:`n`n$_`n`n$($_.ScriptStackTrace)",
        'SEPA Validator - Fehler',
        'OK',
        'Error'
    )
}
