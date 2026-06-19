# Datei-Übersicht: Verwendungszwecke (Ustrd) + PmtInf-Statistik — Design

**Datum:** 2026-06-19
**Branch:** feature/tauri-rust-rewrite
**Komponente:** Tauri/Svelte-App (`app/`) — Backend-Extraktion + neuer Viewer-Tab

## Ziel

Pro ausgewählter Datei eine Übersicht anzeigen:

1. **Verwendungszweck-Liste** — alle `Ustrd`-Werte (unstrukturierte Remittance-Info)
   der Datei, flach in Dokumentreihenfolge.
2. **PmtInf-Statistik** — Anzahl der `PmtInf`-Blöcke und je Block dessen
   `NbOfTxs`, `CtrlSum`, `PmtTpInf/SvcLvl/Cd` und Ausführungs-/Einzugsdatum
   (`ReqdExctnDt` bzw. `ReqdColltnDt`).

Reine Lese-Extraktion; die Validierung bleibt unangetastet.

## Scope-Entscheidungen

- **Anzeigeort:** Tab-Umschalter „XML | Übersicht" in der Viewer-Leiste; die
  Übersicht ersetzt im Viewer-Bereich die XML-Ansicht.
- **Formate:** pain.001 (Überweisung, Datum `ReqdExctnDt`) und pain.008
  (Lastschrift, Datum `ReqdColltnDt`). Da beide dieselben lokalen Element-Namen
  verwenden, ist die Extraktion generisch über lokale Namen und nimmt das
  vorhandene Datumsfeld. Andere Typen (pain.002/007, camt.054, GBIC-Container)
  haben keine `PmtInf`-Blöcke → Übersicht zeigt „keine Zahlungsblöcke".
- **Ustrd-Liste:** flach, in Dokumentreihenfolge, jede Transaktion einzeln (mit
  Wiederholungen), keine Deduplizierung.
- **Berechnung:** on-demand pro Datei (neuer Tauri-Command), analog zu
  `read_formatted`. Funktioniert für jede wohlgeformte Datei, auch schema-invalide.

## Architektur / Komponenten

### Backend — neues Modul `app/src-tauri/src/payments.rs`

Selbstständiges, fokussiertes Modul: DTOs + Extraktion + Tests.

DTOs (serde, `rename_all = "camelCase"`):

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PmtInfSummary {
    pub nb_of_txs: Option<String>,
    pub ctrl_sum: Option<String>,
    pub svc_lvl_cd: Option<String>,
    pub reqd_date: Option<String>, // ReqdExctnDt oder ReqdColltnDt
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentSummary {
    pub message_type: String,  // aus Root-Namespace, z. B. "pain.001.001.03"
    pub pmt_inf_count: u32,
    pub blocks: Vec<PmtInfSummary>,
    pub ustrd: Vec<String>,    // flach, Dokumentreihenfolge
}
```

Alle Zahlen/Daten als `String`, um die XML-Werte unverändert zu zeigen (kein
Parsen/Runden).

Funktion:

```rust
pub fn extract_payment_summary(path: &Path) -> Result<PaymentSummary, String>
```

Verhalten:
- Liest die Datei und streamt sie mit `quick-xml` (wie `formatting.rs` /
  `validator::detect_namespace`).
- Matcht **lokale** Element-Namen (Namespace-Präfix wird ignoriert), per
  `Event::Start`/`Event::Text`/`Event::End`.
- `message_type`: aus dem Namespace des Root-Elements; der lokale Teil nach dem
  letzten `:` (z. B. `pain.001.001.03`). Fällt auf den Namespace bzw. leer
  zurück, wenn keiner gefunden wird.
- Beim Betreten von `PmtInf` einen neuen `PmtInfSummary` beginnen; beim Verlassen
  abschließen und `pmt_inf_count` erhöhen.
- Innerhalb eines `PmtInf`: das **erste** Vorkommen von `NbOfTxs`, `CtrlSum`,
  `Cd` (unterhalb `PmtTpInf/SvcLvl`) und das Datum festhalten.
  - **Datum:** beim Betreten von `ReqdExctnDt` oder `ReqdColltnDt` den Textwert
    erfassen; ist er als `Dt`/`DtTm` verschachtelt (pain.x.09), den Text des
    inneren Elements nehmen (tiefster Textknoten unter dem Datumselement).
  - **SvcLvl/Cd:** nur `Cd` unterhalb `SvcLvl` (nicht `Prtry`).
- Jedes `Ustrd` (Text) in Dokumentreihenfolge an `ustrd` anhängen.
- Nicht wohlgeformt → `Err(String)`.

> Implementierungs-Hinweis zur Disambiguierung: `Cd` kommt im XML auch außerhalb
> von `SvcLvl` vor. Die Extraktion verfolgt daher einen leichten Element-Pfad-
> Stack (lokale Namen) und liest `Cd` nur, wenn der direkte Elternpfad
> `…/PmtTpInf/SvcLvl` ist. Analog wird `NbOfTxs`/`CtrlSum` nur auf
> `PmtInf`-Ebene gelesen (nicht das gleichnamige `GrpHdr`-Feld) und `Ustrd` nur
> innerhalb von `PmtInf`.

### Backend — Command in `app/src-tauri/src/commands.rs`

```rust
#[tauri::command]
pub fn read_payment_summary(path: String) -> Result<PaymentSummary, String> {
    crate::payments::extract_payment_summary(std::path::Path::new(&path))
}
```

Registrierung: `mod payments;` in `lib.rs` und `read_payment_summary` in den
`invoke_handler`-`generate_handler!`-Aufruf aufnehmen (neben den bestehenden
Commands).

### Frontend

- **`app/src/lib/types.ts`:** TypeScript-Typen `PmtInfSummary` und
  `PaymentSummary` passend zu den serde-DTOs.
- **`app/src/lib/api.ts`:** Wrapper
  `readPaymentSummary(path: string): Promise<PaymentSummary>` (ruft
  `invoke("read_payment_summary", { path })`), analog zu `readFormatted`.
- **`app/src/App.svelte`:** lokaler State `viewerTab: "xml" | "summary"`. Die
  Viewer-Leiste bekommt links einen Tab-Umschalter (zwei Buttons „XML" /
  „Übersicht", aktiver Button hervorgehoben). Die bestehenden Buttons
  `Search` / `Collapse all` / `Expand all` werden nur im `xml`-Tab gezeigt. Der
  Viewer-Body rendert `{#if viewerTab === "xml"}<CodeViewer/>{:else}<SummaryView/>{/if}`.
- **`app/src/lib/SummaryView.svelte`** (neu): reagiert auf `$selectedResult?.path`,
  ruft `readPaymentSummary(path)` und rendert:
  1. Kopfzeile „N PmtInf-Blöcke" + eine Tabelle mit Spalten
     `NbOfTxs | CtrlSum | SvcLvl/Cd | Datum`, eine Zeile je Block.
  2. darunter eine nummerierte Liste aller `Ustrd` in Dokumentreihenfolge.
  - Leerwerte werden als „—" dargestellt.
- **`app/src/app.css`:** Stile für den Tab-Umschalter und `SummaryView`
  (Tabelle, Liste, scrollbarer Bereich, der die Höhe unter der Leiste füllt —
  wie `.codehost`).

## Datenfluss

Dateiauswahl → `selectedResult` (bestehend). Im `summary`-Tab ist `SummaryView`
gemountet und lädt bei Pfadwechsel die Übersicht über den neuen Command. Reine
Frontend-Reaktion; kein Streaming nötig (einzelner Aufruf pro Datei).

## Koexistenz mit bestehenden Features

XML-Suche/Folding und die Such-/Falt-Buttons bleiben unverändert und gelten nur
im XML-Tab. Validierung, Log, Fehlerzeilen-Sprung sind nicht betroffen.
`SummaryView` und `CodeViewer` sind nie gleichzeitig gemountet.

## Fehlerfälle

- **Nicht wohlgeformt:** Command liefert `Err`; `SummaryView` zeigt „Datei konnte
  nicht als XML gelesen werden".
- **Keine `PmtInf`-Blöcke** (camt/pain.002/007/Container): Tabelle entfällt,
  Hinweis „Keine Zahlungsblöcke in dieser Datei".
- **Keine `Ustrd`:** Liste entfällt, Hinweis „Keine Verwendungszwecke".
- **Keine Datei gewählt:** `SummaryView` zeigt einen neutralen Leerzustand.

## Test / Verifikation

- **Backend (`payments.rs`):** Unit-Tests mit Inline-Fixtures —
  ein pain.001 (2 PmtInf-Blöcke, mehrere `Ustrd`) prüft `pmt_inf_count`,
  Reihenfolge und Werte von `nb_of_txs`/`ctrl_sum`/`svc_lvl_cd`/`reqd_date`
  (= `ReqdExctnDt`) sowie die flache `ustrd`-Reihenfolge; ein pain.008 prüft,
  dass `reqd_date` aus `ReqdColltnDt` kommt; ein Nicht-Zahlungs-Dokument liefert
  `pmt_inf_count == 0` und leere Listen; ein verschachteltes `ReqdExctnDt/Dt`
  (pain.x.09) wird korrekt aufgelöst.
- `cd app/src-tauri && cargo test` (neue Tests + 20 bestehende) grün.
- `cd app && npm run check` (svelte-check + tsc) grün.
- Manueller GUI-Check separat (Tab-Umschalter, Tabelle, Ustrd-Liste, Leerfälle).

## Bewusst weggelassen (YAGNI)

- Export der Übersicht (bestehender TXT/CSV-Export bleibt validierungsbezogen).
- GrpHdr-Gesamtsummen / Quersummen-Abgleich.
- Strukturierte Remittance-Info (`RmtInf/Strd`), `Prtry`-Service-Level.
- Bearbeiten, Sortieren oder Filtern der Liste/Tabelle.
- Persistenz oder Caching der Übersicht.
