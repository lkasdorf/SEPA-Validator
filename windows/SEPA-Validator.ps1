#Requires -Version 5.1
<#
.SYNOPSIS
    SEPA XML Validator - Windows GUI Tool
.DESCRIPTION
    Validates SEPA XML files against ISO 20022 XSD schemas.
    Supports drag & drop, file/folder selection, and batch validation.
    Schemas can be embedded (EXE build) or loaded from a "schemas\" subfolder.
#>

try {

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

$ErrorActionPreference = 'Stop'

# --- Schema-Konfiguration ---

# Namespace -> XSD-Datei
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

# Eingebettete Schemas (wird vom Build-Script befuellt)
# @@EMBEDDED_SCHEMAS@@

# Schema-Verzeichnis bestimmen: eingebettet -> Temp, sonst -> schemas/ neben Script
if ($EmbeddedSchemas) {
    $SchemaDir = Join-Path ([System.IO.Path]::GetTempPath()) 'SEPA-Validator-Schemas'
    if (-not (Test-Path $SchemaDir)) { [void](New-Item -ItemType Directory -Path $SchemaDir) }
    foreach ($entry in $EmbeddedSchemas.GetEnumerator()) {
        $targetPath = Join-Path $SchemaDir $entry.Key
        if (-not (Test-Path $targetPath)) {
            $bytes = [System.Convert]::FromBase64String($entry.Value)
            $ms = New-Object System.IO.MemoryStream(,$bytes)
            $gs = New-Object System.IO.Compression.GZipStream($ms, [System.IO.Compression.CompressionMode]::Decompress)
            $sr = New-Object System.IO.StreamReader($gs, [System.Text.Encoding]::UTF8)
            [System.IO.File]::WriteAllText($targetPath, $sr.ReadToEnd(), [System.Text.Encoding]::UTF8)
            $sr.Close(); $gs.Close(); $ms.Close()
        }
    }
} else {
    $ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
    $SchemaDir = Join-Path $ScriptDir 'schemas'
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
        File      = [System.IO.Path]::GetFileName($FilePath)
        Path      = $FilePath
        Namespace = ''
        Schema    = ''
        Status    = ''
        Messages  = [System.Collections.Generic.List[PSCustomObject]]::new()
    }

    if (-not (Test-Path $FilePath)) {
        $result.Status = 'ERROR'
        $result.Messages.Add([PSCustomObject]@{ Type = 'Error'; Text = 'File not found.' })
        return $result
    }

    $ns = Get-XmlNamespace -FilePath $FilePath
    if (-not $ns) {
        $result.Status = 'ERROR'
        $result.Messages.Add([PSCustomObject]@{ Type = 'Error'; Text = 'No XML namespace detected. File may not be valid XML.' })
        return $result
    }
    $result.Namespace = $ns

    $schemaFile = $null
    if ($SchemaMap.Contains($ns)) {
        $schemaFile = Join-Path $SchemaDir $SchemaMap[$ns]
    }

    if (-not $schemaFile -or -not (Test-Path $schemaFile)) {
        $result.Status = 'NO SCHEMA'
        $result.Messages.Add([PSCustomObject]@{
            Type = 'Warning'
            Text = "No matching schema for namespace: $ns"
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
            $result.Status = 'ERROR'
            $result.Messages.Add([PSCustomObject]@{ Type = 'Error'; Text = "Failed to load schema: $_" })
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

    $messages = $result.Messages
    $readerSettings.add_ValidationEventHandler([System.Xml.Schema.ValidationEventHandler]{
        param($sender, [System.Xml.Schema.ValidationEventArgs]$e)
        $type = if ($e.Severity -eq [System.Xml.Schema.XmlSeverityType]::Error) { 'Error' } else { 'Warning' }
        $location = ''
        if ($e.Exception -and $e.Exception.LineNumber -gt 0) {
            $location = " (Line $($e.Exception.LineNumber), Col $($e.Exception.LinePosition))"
        }
        $messages.Add([PSCustomObject]@{
            Type = $type
            Text = "$($e.Message)$location"
        })
    })

    try {
        $reader = [System.Xml.XmlReader]::Create($FilePath, $readerSettings)
        while ($reader.Read()) { }
        $reader.Close()
    } catch {
        $result.Messages.Add([PSCustomObject]@{ Type = 'Error'; Text = "XML read error: $_" })
    }

    $errors = @($result.Messages | Where-Object { $_.Type -eq 'Error' }).Count
    $warnings = @($result.Messages | Where-Object { $_.Type -eq 'Warning' }).Count

    if ($errors -gt 0) {
        $result.Status = "INVALID ($errors errors, $warnings warnings)"
    } elseif ($warnings -gt 0) {
        $result.Status = "WARNINGS ($warnings)"
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
$headerPanel.Height = 44
$headerPanel.BackColor = [System.Drawing.Color]::FromArgb(0, 90, 158)

$titleLabel = New-Object System.Windows.Forms.Label
$titleLabel.Text = 'SEPA XML Validator'
$titleLabel.ForeColor = [System.Drawing.Color]::White
$titleLabel.Font = New-Object System.Drawing.Font('Segoe UI', 13, [System.Drawing.FontStyle]::Bold)
$titleLabel.AutoSize = $true
$titleLabel.Location = New-Object System.Drawing.Point(12, 10)
$headerPanel.Controls.Add($titleLabel)

$versionLabel = New-Object System.Windows.Forms.Label
$versionLabel.Text = 'v1.0.0'
$versionLabel.ForeColor = [System.Drawing.Color]::FromArgb(180, 210, 240)
$versionLabel.Font = New-Object System.Drawing.Font('Segoe UI', 9)
$versionLabel.AutoSize = $true
$versionLabel.Location = New-Object System.Drawing.Point(220, 16)
$headerPanel.Controls.Add($versionLabel)

$form.Controls.Add($headerPanel)

# --- Toolbar ---
$toolPanel = New-Object System.Windows.Forms.Panel
$toolPanel.Dock = 'Top'
$toolPanel.Height = 40
$toolPanel.Padding = New-Object System.Windows.Forms.Padding(12, 4, 12, 4)

$btnDatei = New-Object System.Windows.Forms.Button
$btnDatei.Text = 'Select Files...'
$btnDatei.Size = New-Object System.Drawing.Size(120, 30)
$btnDatei.Location = New-Object System.Drawing.Point(12, 5)
$btnDatei.FlatStyle = 'Flat'
$btnDatei.BackColor = [System.Drawing.Color]::FromArgb(0, 120, 212)
$btnDatei.ForeColor = [System.Drawing.Color]::White
$btnDatei.FlatAppearance.BorderSize = 0
$btnDatei.Cursor = [System.Windows.Forms.Cursors]::Hand
$toolPanel.Controls.Add($btnDatei)

$btnOrdner = New-Object System.Windows.Forms.Button
$btnOrdner.Text = 'Select Folder...'
$btnOrdner.Size = New-Object System.Drawing.Size(120, 30)
$btnOrdner.Location = New-Object System.Drawing.Point(142, 5)
$btnOrdner.FlatStyle = 'Flat'
$btnOrdner.BackColor = [System.Drawing.Color]::FromArgb(0, 120, 212)
$btnOrdner.ForeColor = [System.Drawing.Color]::White
$btnOrdner.FlatAppearance.BorderSize = 0
$btnOrdner.Cursor = [System.Windows.Forms.Cursors]::Hand
$toolPanel.Controls.Add($btnOrdner)

$btnExport = New-Object System.Windows.Forms.Button
$btnExport.Text = 'Export Results...'
$btnExport.Size = New-Object System.Drawing.Size(130, 30)
$btnExport.Location = New-Object System.Drawing.Point(272, 5)
$btnExport.FlatStyle = 'Flat'
$btnExport.BackColor = [System.Drawing.Color]::FromArgb(100, 100, 100)
$btnExport.ForeColor = [System.Drawing.Color]::White
$btnExport.FlatAppearance.BorderSize = 0
$btnExport.Cursor = [System.Windows.Forms.Cursors]::Hand
$btnExport.Enabled = $false
$toolPanel.Controls.Add($btnExport)

$form.Controls.Add($toolPanel)

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
$splitContainer.SplitterDistance = 160
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

[void]$grid.Columns.Add('File', 'File')
[void]$grid.Columns.Add('Namespace', 'Namespace')
[void]$grid.Columns.Add('Schema', 'Schema')
[void]$grid.Columns.Add('Status', 'Status')

$grid.Columns['File'].FillWeight = 25
$grid.Columns['Namespace'].FillWeight = 35
$grid.Columns['Schema'].FillWeight = 20
$grid.Columns['Status'].FillWeight = 20
$grid.AllowDrop = $true

# Drop hint overlay on the grid
$dropLabel = New-Object System.Windows.Forms.Label
$dropLabel.Text = "Drop XML files here`nor use the buttons above"
$dropLabel.Dock = 'Fill'
$dropLabel.TextAlign = 'MiddleCenter'
$dropLabel.Font = New-Object System.Drawing.Font('Segoe UI', 10)
$dropLabel.ForeColor = [System.Drawing.Color]::FromArgb(160, 160, 160)
$dropLabel.BackColor = [System.Drawing.Color]::White
$dropLabel.AllowDrop = $true

$splitContainer.Panel1.Controls.Add($dropLabel)
$splitContainer.Panel1.Controls.Add($grid)
$dropLabel.BringToFront()

# Detail-Ansicht
$detailBox = New-Object System.Windows.Forms.RichTextBox
$detailBox.Dock = 'Fill'
$detailBox.ReadOnly = $true
$detailBox.Font = New-Object System.Drawing.Font('Consolas', 9.5)
$detailBox.BackColor = [System.Drawing.Color]::FromArgb(30, 30, 30)
$detailBox.ForeColor = [System.Drawing.Color]::FromArgb(220, 220, 220)
$detailBox.BorderStyle = 'None'
$detailBox.Text = 'Select a file above to view validation details.'

$splitContainer.Panel2.Controls.Add($detailBox)
$form.Controls.Add($splitContainer)

# --- Statusleiste ---
$statusStrip = New-Object System.Windows.Forms.StatusStrip
$statusStrip.SizingGrip = $true

$statusLabel = New-Object System.Windows.Forms.ToolStripStatusLabel
$statusLabel.Text = 'Ready'
$statusLabel.Spring = $true
$statusLabel.TextAlign = 'MiddleLeft'

$sepA1 = New-Object System.Windows.Forms.ToolStripSeparator

$ghLink = New-Object System.Windows.Forms.ToolStripStatusLabel
$ghLink.Text = 'GitHub'
$ghLink.IsLink = $true
$ghLink.LinkColor = [System.Drawing.Color]::FromArgb(0, 102, 180)
$ghLink.ActiveLinkColor = [System.Drawing.Color]::FromArgb(0, 70, 130)
$ghLink.VisitedLinkColor = [System.Drawing.Color]::FromArgb(0, 102, 180)
$ghLink.add_Click({ [System.Diagnostics.Process]::Start('https://github.com/lkasdorf/SEPA-Validator') })

$sepA2 = New-Object System.Windows.Forms.ToolStripSeparator

$licenseLabel = New-Object System.Windows.Forms.ToolStripStatusLabel
$licenseLabel.Text = 'MIT License'
$licenseLabel.ForeColor = [System.Drawing.Color]::FromArgb(100, 100, 100)

$sepA3 = New-Object System.Windows.Forms.ToolStripSeparator

$versionStatus = New-Object System.Windows.Forms.ToolStripStatusLabel
$versionStatus.Text = 'v1.0.0'
$versionStatus.ForeColor = [System.Drawing.Color]::FromArgb(100, 100, 100)

[void]$statusStrip.Items.Add($statusLabel)
[void]$statusStrip.Items.Add($sepA1)
[void]$statusStrip.Items.Add($ghLink)
[void]$statusStrip.Items.Add($sepA2)
[void]$statusStrip.Items.Add($licenseLabel)
[void]$statusStrip.Items.Add($sepA3)
[void]$statusStrip.Items.Add($versionStatus)

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

    $rowIndex = $grid.Rows.Add($Result.File, $Result.Namespace, $Result.Schema, $Result.Status)
    $row = $grid.Rows[$rowIndex]

    switch -Wildcard ($Result.Status) {
        'OK'          { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(16, 124, 16) }
        'INVALID*'    { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(196, 43, 28) }
        'WARNINGS*'   { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(157, 93, 0) }
        'NO SCHEMA'   { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(157, 93, 0) }
        'ERROR'       { $row.DefaultCellStyle.ForeColor = [System.Drawing.Color]::FromArgb(196, 43, 28) }
    }
}

function Show-Detail {
    param([int]$Index)

    if ($Index -lt 0 -or $Index -ge $script:Results.Count) { return }
    $r = $script:Results[$Index]

    $detailBox.Clear()

    $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(86, 156, 214)
    $detailBox.AppendText("File: $($r.Path)`n")

    $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(220, 220, 220)
    $detailBox.AppendText("Namespace: $($r.Namespace)`n")
    $detailBox.AppendText("Schema: $($r.Schema)`n")

    $statusColor = switch -Wildcard ($r.Status) {
        'OK'         { [System.Drawing.Color]::FromArgb(78, 201, 176) }
        'INVALID*'   { [System.Drawing.Color]::FromArgb(244, 71, 71) }
        'WARNINGS*'  { [System.Drawing.Color]::FromArgb(255, 200, 50) }
        default      { [System.Drawing.Color]::FromArgb(255, 200, 50) }
    }
    $detailBox.SelectionColor = $statusColor
    $detailBox.AppendText("Status: $($r.Status)`n")

    $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(220, 220, 220)
    $detailBox.AppendText("`n")

    if ($r.Messages.Count -eq 0) {
        $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(78, 201, 176)
        $detailBox.AppendText("No errors or warnings.`n")
    } else {
        $nr = 1
        foreach ($m in $r.Messages) {
            if ($m.Type -eq 'Error') {
                $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(244, 71, 71)
                $detailBox.AppendText("[$nr] ERROR: ")
            } else {
                $detailBox.SelectionColor = [System.Drawing.Color]::FromArgb(255, 200, 50)
                $detailBox.AppendText("[$nr] WARNING: ")
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
            'No XML files found.',
            'Notice',
            'OK',
            'Information'
        )
        return
    }

    $dropLabel.Visible = $false
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
        Update-StatusBar "Validating $nr / $total : $fileName"
        $progressBar.Value = $nr
        [System.Windows.Forms.Application]::DoEvents()

        try {
            $result = Test-SepaXml -FilePath $file
        } catch {
            $result = [PSCustomObject]@{
                File      = $fileName
                Path      = $file
                Namespace = ''
                Schema    = ''
                Status    = 'ERROR'
                Messages  = [System.Collections.Generic.List[PSCustomObject]]::new()
            }
            $result.Messages.Add([PSCustomObject]@{ Type = 'Error'; Text = "Unexpected error: $_" })
        }
        $script:Results.Add($result)
        Add-ResultToGrid -Result $result

        switch -Wildcard ($result.Status) {
            'OK'          { $ok++ }
            'INVALID*'    { $fail++ }
            'WARNINGS*'   { $warn++ }
            'NO SCHEMA'   { $noSchema++ }
            'ERROR'       { $fail++ }
        }
    }

    $progressPanel.Visible = $false
    $form.Cursor = [System.Windows.Forms.Cursors]::Default
    Update-StatusBar "$total files | OK: $ok | Invalid: $fail | Warnings: $warn | No Schema: $noSchema"
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
    }
}

$script:HandleDragLeave = {
}

$script:HandleDragDrop = {
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
    $dlg.Title = 'Select SEPA XML Files'
    $dlg.Filter = 'XML Files (*.xml)|*.xml|All Files (*.*)|*.*'
    $dlg.Multiselect = $true
    if ($dlg.ShowDialog() -eq 'OK') {
        Start-Validation -Files $dlg.FileNames
    }
})

$btnOrdner.add_Click({
    $dlg = New-Object System.Windows.Forms.FolderBrowserDialog
    $dlg.Description = 'Select folder containing SEPA XML files'
    $dlg.ShowNewFolderButton = $false
    if ($dlg.ShowDialog() -eq 'OK') {
        $files = Get-ChildItem -Path $dlg.SelectedPath -Filter '*.xml' -Recurse -File | ForEach-Object { $_.FullName }
        Start-Validation -Files $files
    }
})

$btnExport.add_Click({
    $dlg = New-Object System.Windows.Forms.SaveFileDialog
    $dlg.Title = 'Export Validation Results'
    $dlg.Filter = 'Text Files (*.txt)|*.txt|All Files (*.*)|*.*'
    $dlg.FileName = "SEPA_Validation_$(Get-Date -Format 'yyyyMMdd_HHmmss')"
    if ($dlg.ShowDialog() -eq 'OK') {
        $sb = New-Object System.Text.StringBuilder
        $total = $script:Results.Count
        $okCount = @($script:Results | Where-Object { $_.Status -eq 'OK' }).Count
        $failCount = $total - $okCount

        [void]$sb.AppendLine("SEPA XML Validation - $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')")
        [void]$sb.AppendLine("$total files checked | OK: $okCount | Invalid: $failCount")
        [void]$sb.AppendLine(('=' * 80))

        foreach ($r in $script:Results) {
            [void]$sb.AppendLine('')
            [void]$sb.AppendLine("File: $($r.Path)")
            [void]$sb.AppendLine("Namespace: $($r.Namespace)")
            [void]$sb.AppendLine("Schema: $($r.Schema)")
            [void]$sb.AppendLine("Status: $($r.Status)")

            if ($r.Messages.Count -gt 0) {
                [void]$sb.AppendLine('')
                $nr = 1
                foreach ($m in $r.Messages) {
                    $label = if ($m.Type -eq 'Error') { 'ERROR' } else { 'WARNING' }
                    [void]$sb.AppendLine("[$nr] ${label}: $($m.Text)")
                    [void]$sb.AppendLine('')
                    $nr++
                }
            }

            [void]$sb.AppendLine(('-' * 80))
        }

        [System.IO.File]::WriteAllText($dlg.FileName, $sb.ToString(), [System.Text.Encoding]::UTF8)
        Update-StatusBar "Export saved: $($dlg.FileName)"
    }
})

# --- Schema-Verzeichnis pruefen ---
if (-not (Test-Path $SchemaDir)) {
    [System.Windows.Forms.MessageBox]::Show(
        "Schema folder not found:`n$SchemaDir`n`nPlease create a 'schemas' folder next to this script and place the XSD files inside.",
        'Schema Folder Missing',
        'OK',
        'Warning'
    )
}

# --- Start ---
[void]$form.ShowDialog()

} catch {
    [System.Windows.Forms.MessageBox]::Show(
        "Unexpected error:`n`n$_`n`n$($_.ScriptStackTrace)",
        'SEPA Validator - Error',
        'OK',
        'Error'
    )
}
