# Verwendungszweck: Herkunft (InstrId/EndToEndId) + CSV-Export — Design

**Datum:** 2026-06-20
**Branch:** feature/tauri-rust-rewrite
**Komponente:** Tauri/Svelte-App (`app/`) — Payment-Extraktion + Verwendungszweck-Tab

## Ziel

Im **Verwendungszweck**-Tab je Eintrag anzeigen, **aus welchem Datensatz** er stammt
(Herkunft = `InstrId`, ersatzweise `EndToEndId`), und die Tabelle als **CSV**
exportierbar machen.

## Scope-Entscheidungen

- **Herkunft:** eine Spalte je Transaktion = `InstrId` (`PmtId/InstrId`); fehlt sie
  (optional), `EndToEndId` (`PmtId/EndToEndId`, Pflichtfeld). Beide werden im Backend
  erfasst, die Anzeige nutzt den Fallback.
- **Anzeige:** der Verwendungszweck-Tab wird eine **Tabelle** (`# · Herkunft ·
  Verwendungszweck`) statt der bisherigen nummerierten Liste. Banner bleibt.
- **Export:** nur **CSV** (`;`-getrennt, deutsche Excel-Konvention), nur die aktuell
  gewählte Datei. Fehlender Verwendungszweck → **leere** CSV-Zelle (Warnung nur in der UI).
- Kein TXT-Export, kein dateiübergreifender Export, keine getrennten InstrId/EndToEndId-
  Spalten, kein Export der Übersicht/PmtInf-Tabelle (YAGNI).

## Architektur / Komponenten

### Backend — `app/src-tauri/src/payments.rs`

- `RemittanceEntry` erhält zwei Felder (serde camelCase):
  ```rust
  pub struct RemittanceEntry {
      pub instr_id: Option<String>,      // PmtId/InstrId
      pub end_to_end_id: Option<String>, // PmtId/EndToEndId
      pub ustrd: Option<String>,
  }
  ```
- Der Transaktions-Akkumulator wird von `Option<Vec<String>>` (nur Ustrd) auf eine kleine
  Struktur erweitert, die zusätzlich `instr_id`/`end_to_end_id` (jeweils erstes Vorkommen)
  hält. Beim Betreten von `CdtTrfTxInf`/`DrctDbtTxInf` wird sie neu angelegt; beim
  Verlassen entsteht `RemittanceEntry { instr_id, end_to_end_id, ustrd }` (ustrd = join der
  nicht-leeren Ustrd oder `None`, wie bisher).
- Erfassung innerhalb einer Transaktion: `top == "InstrId" && parent == "PmtId"` →
  `instr_id`; `top == "EndToEndId" && parent == "PmtId"` → `end_to_end_id`. (PmtId kommt
  nur auf Transaktionsebene vor → keine Disambiguierung nötig, aber die `current_tx`-Guard
  stellt sicher, dass nur innerhalb einer Transaktion erfasst wird.)
- Tests erweitert: eine Transaktion mit `InstrId`+`EndToEndId` (beide gesetzt), eine nur
  mit `EndToEndId` (InstrId `None`); plus eine bestehende ohne PmtId (beide `None`).

### Frontend

- **`types.ts`:** `RemittanceEntry` um `instrId: string | null` und
  `endToEndId: string | null` ergänzen.
- **`RemittanceView.svelte`:** die `<ol>`-Liste wird eine `<table>`:
  - Kopf: `#`, `Herkunft`, `Verwendungszweck`.
  - Zeile je Transaktion: Index; Herkunft = `t.instrId ?? t.endToEndId ?? "—"`;
    Verwendungszweck = `t.ustrd` bzw. rot hervorgehoben „⚠ kein Verwendungszweck", wenn
    `t.ustrd == null`.
  - Banner „⚠ N von M Transaktionen ohne Verwendungszweck" bleibt (über der Tabelle).
  - Ein **„Export CSV"**-Knopf über der Tabelle, deaktiviert wenn keine Transaktionen;
    ruft `exportRemittanceCsv(data.transactions, $selectedResult.file)`.
  - Zustände (kein Doc / error / loading / keine Transaktionen) wie bisher.
- **`export.ts`:** neue Funktion
  `exportRemittanceCsv(transactions: RemittanceEntry[], sourceFile: string): Promise<void>`:
  - `save`-Dialog mit `defaultPath` `Verwendungszweck_<sourceFile-ohne-ext>_<stamp>.csv`,
    Filter CSV.
  - Inhalt: Kopfzeile `#;Herkunft;Verwendungszweck`, dann je Transaktion
    `index;esc(instrId ?? endToEndId ?? "");esc((ustrd ?? "").replace(/\r?\n/g, " "))`.
    `esc` wie im bestehenden `exportCsv` (`"`-quoten, `"`→`""`).
  - via `writeTextFile` schreiben. Mirror des bestehenden `exportCsv`.

## Datenfluss

`read_payment_summary` liefert je Transaktion jetzt zusätzlich `instrId`/`endToEndId`.
`RemittanceView` liest sie aus dem `paymentSummary`-Store, zeigt die Herkunft-Spalte und
exportiert die Transaktionen der aktuell gewählten Datei als CSV.

## Fehler-/Leerfälle

- **Keine Transaktionen:** „Keine Transaktionen in dieser Datei."; Export-Knopf deaktiviert.
- **Fehlende Herkunft** (weder InstrId noch EndToEndId — selten, da EndToEndId Pflicht):
  Zelle „—" in der UI, leere Zelle im CSV.
- **Fehlender Verwendungszweck:** rote Markierung in der UI, leere Verwendungszweck-Zelle
  im CSV.
- **Speichern abgebrochen:** `save` liefert null → Export bricht still ab (wie bestehend).

## Test / Verifikation

- **Backend:** erweiterte `payments.rs`-Tests (InstrId+EndToEndId erfasst; Fallback-Fall
  nur EndToEndId; ohne PmtId beide `None`). `cd app/src-tauri && cargo test` grün.
- **Frontend:** `cd app && npm run check` grün.
- Manuell: Verwendungszweck-Tab zeigt Herkunft-Spalte (InstrId bzw. EndToEndId); „Export
  CSV" erzeugt eine in Excel öffenbare Datei mit den drei Spalten.

## Bewusst weggelassen (YAGNI)

- TXT-Export; dateiübergreifender Export; Export der Übersicht/PmtInf-Tabelle.
- Getrennte InstrId- und EndToEndId-Spalten.
- Konfigurierbares Trennzeichen / Encoding-Optionen.
