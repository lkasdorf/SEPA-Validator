# SEPA-Validator

Windows-GUI-Tool zur Validierung von SEPA-XML-Zahlungsdateien gegen ISO 20022 XSD-Schemas.

![PowerShell](https://img.shields.io/badge/PowerShell-5.1+-blue) ![Windows](https://img.shields.io/badge/Windows-10%2F11-blue) ![License](https://img.shields.io/badge/License-Private-lightgrey)

## Features

- **Drag & Drop** - XML-Dateien oder Ordner direkt ins Fenster ziehen
- **Datei-/Ordnerauswahl** - ueber Dialoge einzelne Dateien oder ganze Verzeichnisse waehlen
- **Automatische Schema-Erkennung** - erkennt den XML-Namespace und waehlt das passende XSD
- **Detaillierte Fehlermeldungen** - mit Zeilennummer und Spaltenangabe
- **Farbcodierte Ergebnisse** - Gruen (OK), Rot (Fehler), Gelb (Warnungen)
- **CSV-Export** - Validierungsergebnisse als CSV speichern
- **Keine Installation noetig** - laeuft mit PowerShell 5.1 (auf jedem Windows 10/11 vorinstalliert)

## Unterstuetzte SEPA-Formate

| Format | Schema | Beschreibung |
|--------|--------|-------------|
| pain.001.001.03 | Credit Transfer v03 | Ueberweisungen |
| pain.001.001.09 | Credit Transfer v09 | Ueberweisungen (aktuell) |
| pain.002.001.10 | Payment Status v10 | Statusberichte |
| pain.007.001.09 | Payment Reversal v09 | Rueckgaben |
| pain.008.001.02 | Direct Debit v02 | Lastschriften |
| pain.008.001.08 | Direct Debit v08 | Lastschriften (aktuell) |
| camt.054.001.08 | Bank-to-Customer Debit/Credit Notification | Kontoauszuege |

## Schnellstart

### 1. Herunterladen

Den Ordner `windows/` auf den Windows-Rechner kopieren.

### 2. Schemas einrichten

`setup.cmd` doppelklicken - kopiert die XSD-Schemas in den `schemas/`-Unterordner.

Alternativ manuell: einen Ordner `schemas/` neben `SEPA-Validator.ps1` anlegen und die XSD-Dateien hineinkopieren.

### 3. Starten

`SEPA-Validator.cmd` doppelklicken.

## Bedienung

1. **Dateien laden** - per Drag & Drop, "Dateien waehlen..." oder "Ordner waehlen..."
2. **Ergebnisse pruefen** - Datei in der oberen Liste anklicken, Details erscheinen unten
3. **Exportieren** - "Ergebnis exportieren..." speichert eine CSV-Datei mit allen Ergebnissen

## Verzeichnisstruktur

```
windows/
  SEPA-Validator.cmd    # Starter (Doppelklick)
  SEPA-Validator.ps1    # Hauptanwendung
  setup.cmd             # Einmalig: kopiert XSD-Schemas
  schemas/              # XSD-Schema-Dateien (nach Setup)
```

## Voraussetzungen

- Windows 10 oder 11
- PowerShell 5.1+ (vorinstalliert)
- Keine Administratorrechte erforderlich

## Eigene Schemas hinzufuegen

1. XSD-Datei in den `schemas/`-Ordner kopieren
2. In `SEPA-Validator.ps1` den Namespace in die `$SchemaMap` eintragen:

```powershell
$SchemaMap = [ordered]@{
    'urn:iso:std:iso:20022:tech:xsd:pain.001.001.03' = 'pain.001.001.03.xsd'
    # ... weitere Eintraege ...
    'urn:mein:namespace'                              = 'mein_schema.xsd'
}
```
