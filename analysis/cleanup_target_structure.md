# Vorschlag: Bereinigte Zielstruktur

## Ziel
- Eindeutige, valide XML-Dateien in produktionsnaher Ablage.
- Konfliktkopien und Dubletten getrennt halten.
- Fehlerhafte Dateien klar von validen trennen.

## Struktur
- `to_check/inbox/`
  - Neu eingehende Dateien (ungeprüft)
- `to_check/valid/`
  - XSD-valide Dateien
- `to_check/invalid/`
  - Nicht valide Dateien mit offenen Korrekturen
- `to_check/duplicates/`
  - Hash-identische Kopien (`(1)`, `DownloadConflict`, etc.)
- `to_check/archive/`
  - Historische, nicht mehr aktive Stände

## Dateinamenskonvention
- `<YYYYMMDD>_<FLOW>_<TYPE>_<COUNTERPARTY>_<VERSION>.xml`
- Beispiele:
  - `20250121_P2S_CT_DOCMORRIS_v1.xml`
  - `20250121_P2S_DD_DOCMORRIS_v2.xml`

## Aufräumregeln
- `DownloadConflict` und ` (1)` nie in `valid/` belassen.
- Bei hash-identischen Gruppen genau 1 Datei als kanonisch halten.
- `Sperr_*.xml` nur behalten, wenn inhaltlich unterschiedlich; sonst in `duplicates/`.
- `*.Zone.Identifier` aus `Reference/` löschen oder ignorieren (nur Download-Metadaten).

## Minimalprozess
1. Neue Datei nach `inbox/`.
2. XSD-Validierung.
3. Bei Erfolg: `valid/`; bei Fehler: `invalid/` + Ticket/Kommentar.
4. Täglicher Dublettenlauf (`sha256sum`) und Bereinigung nach Keep-Regel.
