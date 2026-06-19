# Verwendungszweck-Tab, erweiterte Übersicht & Viewer-Bugfix — Design

**Datum:** 2026-06-19
**Branch:** feature/tauri-rust-rewrite
**Komponente:** Tauri/Svelte-App (`app/`) — Backend-Extraktion + Viewer-Tabs

## Ziel

Aufbauend auf der bestehenden Datei-Übersicht (PmtInf-Statistik + Ustrd):

1. **Bugfix:** Wechsel Übersicht → XML zeigt eine leere XML-Ansicht. Beheben.
2. **Verwendungszweck** in einen **eigenen Tab** (getrennt von der PmtInf-Statistik),
   als Liste **je Transaktion**, mit **Warnung** bei leerem/fehlendem Verwendungszweck.
3. **Übersicht erweitern:** `LclInstrm` und `SeqTp` je PmtInf-Block; zusätzlich ein
   **Cdtr-Block** (Gläubiger) am Anfang.

## Bug-Analyse (Punkt 1)

`App.svelte` schaltet den Viewer-Körper per `{#if viewerTab === "xml"}<CodeViewer/>{:else}…`.
Beim Wechsel zur Übersicht wird `CodeViewer` ausgehängt und `view.destroy()` läuft
(CodeViewer.svelte `onMount`-Cleanup). Beim Zurückwechseln ist es eine **frische**
Instanz: der reaktive Lade-Block `$: void loadFor($selectedResult?.path, …)` läuft bei
der Initialisierung **bevor** `onMount` den neuen `EditorView` erstellt, bricht wegen
`if (!view || !path) return` ab — und läuft danach nicht erneut, weil sich der Pfad
nicht ändert. Ergebnis: leerer Editor.

**Fix:** CodeViewer bleibt dauerhaft gemountet und wird nur per CSS ausgeblendet, wenn
der XML-Tab nicht aktiv ist. Kein Destroy/Recreate → kein Leer-Bug; Scroll-, Falt- und
Suchzustand bleiben über Tab-Wechsel erhalten. `CodeViewer.svelte` selbst bleibt
unverändert; `App.svelte` umschließt es mit einem per CSS schaltbaren Wrapper.

## Scope-Entscheidungen

- **Tabs:** `XML | Übersicht | Verwendungszweck`.
- **Cdtr-Block:** aus dem **ersten** PmtInf (Annahme: ein Gläubiger je Datei). Felder:
  Name, IBAN, BIC, Gläubiger-ID. Kein Adressblock (YAGNI).
- **Verwendungszweck:** Liste **je Transaktion** (`CdtTrfTxInf`/`DrctDbtTxInf`) in
  Dokumentreihenfolge; ersetzt die bisherige flache Ustrd-Liste.
- **Warnung:** „leer" und „fehlt" werden gleich behandelt (= „kein Verwendungszweck").
- **Datenfluss:** ein geteilter Store, eine Extraktion je Datei für beide Summary-Tabs.

## Architektur / Komponenten

### Backend — `app/src-tauri/src/payments.rs` (erweitert)

Neue/erweiterte DTOs (serde, `rename_all = "camelCase"`):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Creditor {
    pub name: Option<String>,        // Cdtr/Nm
    pub iban: Option<String>,        // CdtrAcct/Id/IBAN
    pub bic: Option<String>,         // CdtrAgt/FinInstnId/BIC oder BICFI
    pub creditor_id: Option<String>, // CdtrSchmeId/Id/PrvtId/Othr/Id
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PmtInfSummary {
    pub nb_of_txs: Option<String>,
    pub ctrl_sum: Option<String>,
    pub svc_lvl_cd: Option<String>,
    pub lcl_instrm: Option<String>,  // NEU: PmtTpInf/LclInstrm/Cd
    pub seq_tp: Option<String>,      // NEU: PmtTpInf/SeqTp
    pub reqd_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemittanceEntry {
    pub ustrd: Option<String>, // None = Ustrd fehlt ODER leer
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentSummary {
    pub message_type: String,
    pub pmt_inf_count: u32,
    pub creditor: Option<Creditor>,      // NEU: aus dem ersten PmtInf
    pub blocks: Vec<PmtInfSummary>,
    pub transactions: Vec<RemittanceEntry>, // ersetzt das frühere `ustrd`
}
```

Parser (`extract_payment_summary`) — gleiches Pfad-Stack-Verfahren über lokale
Element-Namen, erweitert um:

- **LclInstrm/Cd:** `top == "Cd"` mit `parent == "LclInstrm"` und Großeltern
  `PmtTpInf` → `lcl_instrm` (erste Belegung). (Das bestehende `SvcLvl/Cd` bleibt:
  `top == "Cd"`, `parent == "SvcLvl"`, Großeltern `PmtTpInf`.)
- **SeqTp:** `top == "SeqTp"` mit `parent == "PmtTpInf"` → `seq_tp` (erste Belegung).
- **Creditor (erstes PmtInf):** ein internes `current_creditor: Creditor` je PmtInf;
  beim Verlassen eines PmtInf, falls `summary.creditor` noch `None` und das
  `current_creditor` mindestens ein gesetztes Feld hat, übernehmen. Felder:
  - `name`: `top == "Nm"`, `parent == "Cdtr"`.
  - `iban`: `top == "IBAN"`, `parent == "Id"`, Großeltern `CdtrAcct`.
  - `bic`: `top in {"BIC","BICFI"}`, `parent == "FinInstnId"`, Großeltern `CdtrAgt`.
  - `creditor_id`: `top == "Id"`, `parent == "Othr"`, und `CdtrSchmeId` ist Vorfahr
    im Stack.
- **Transaktionen:** beim Betreten von `CdtTrfTxInf` oder `DrctDbtTxInf` eine
  `current_tx: Vec<String>` (gesammelte, getrimmte, nicht-leere Ustrd) starten; jedes
  `Ustrd` (wie bisher innerhalb `PmtInf`, jetzt zusätzlich innerhalb einer Transaktion)
  anhängen, sofern nicht leer; beim Verlassen der Transaktion eine `RemittanceEntry`
  pushen mit `ustrd = Some(join("\n"))` falls nicht-leere Werte vorhanden, sonst `None`.

`message_type`, `pmt_inf_count`, `blocks` bleiben wie gehabt; `ustrd` wird durch
`transactions` ersetzt. Tests erweitern: LclInstrm/SeqTp je Block, alle vier
Creditor-Felder, eine Transaktion mit fehlendem Ustrd → `ustrd: None`, eine mit
leerem `<Ustrd></Ustrd>` → `None`, Dokumentreihenfolge der Transaktionen, pain.008
(SeqTp/LclInstrm/Creditor vorhanden), Nicht-Zahlungs-Dokument (leere `transactions`,
`creditor: None`).

Der Tauri-Command `read_payment_summary` bleibt unverändert (gleiche Signatur, neuer
Rückgabe-Inhalt).

### Frontend — geteilter Store

Neues Modul `app/src/lib/paymentSummary.ts`:

```ts
import { writable, get } from "svelte/store";
import { readPaymentSummary } from "./api";
import type { PaymentSummary } from "./types";

export type SummaryState = "idle" | "loading" | "ready" | "error";
export const paymentSummary = writable<{ path: string; state: SummaryState; data: PaymentSummary | null }>(
  { path: "", state: "idle", data: null }
);

export async function loadPaymentSummary(path: string | undefined): Promise<void> {
  if (!path) { paymentSummary.set({ path: "", state: "idle", data: null }); return; }
  const cur = get(paymentSummary);
  if (cur.path === path && (cur.state === "ready" || cur.state === "loading")) return;
  paymentSummary.set({ path, state: "loading", data: null });
  try {
    const data = await readPaymentSummary(path);
    paymentSummary.set({ path, state: "ready", data });
  } catch {
    paymentSummary.set({ path, state: "error", data: null });
  }
}
```

`App.svelte` triggert das Laden, sobald ein Summary-Tab aktiv ist:
`$: if (viewerTab !== "xml") loadPaymentSummary($selectedResult?.path);`

### Frontend — Komponenten

- **`types.ts`:** `Creditor`, `RemittanceEntry`; `PmtInfSummary` um `lclInstrm`/`seqTp`
  erweitert; `PaymentSummary` um `creditor` + `transactions` (statt `ustrd`).
- **`SummaryView.svelte`** (umgebaut → nur Übersicht): liest `$paymentSummary`. Rendert
  oben den Cdtr-Block (Name/IBAN/BIC/Gläubiger-ID; ganz weg, wenn `creditor` null),
  darunter die PmtInf-Tabelle mit Spalten `# · NbOfTxs · CtrlSum · SvcLvl/Cd ·
  LclInstrm · SeqTp · Datum`. Leerwerte als „—". Zustände idle/loading/error/keine
  Blöcke.
- **`RemittanceView.svelte`** (neu): liest `$paymentSummary`. Berechnet
  `M = transactions.length`, `N = transactions.filter(t => t.ustrd == null).length`.
  Banner „⚠ N von M Transaktionen ohne Verwendungszweck" nur wenn `N > 0`. Darunter
  eine nummerierte Liste je Transaktion: Text bei vorhandenem Ustrd, sonst rot
  hervorgehoben „⚠ kein Verwendungszweck". Zustand „keine Transaktionen", wenn leer.
- **`App.svelte`:** drei Tabs; CodeViewer in einem Wrapper, der per
  `class:hidden={viewerTab !== "xml"}` ausgeblendet wird (bleibt gemountet);
  `{#if viewerTab === "summary"}<SummaryView/>{/if}` und
  `{#if viewerTab === "remittance"}<RemittanceView/>{/if}`. Search/Collapse/Expand
  weiterhin nur im XML-Tab.
- **`app.css`:** `.viewer-pane.hidden { display: none; }` (Wrapper füllt sonst die Höhe
  wie bisher `.codehost`); 3-Tab-Stil (aktiver Tab hervorgehoben); Banner- und
  „kein Verwendungszweck"-Warnstil (Akzent `--err`); Cdtr-Block-Stil.

## Datenfluss

Dateiauswahl → `selectedResult`. Bei aktivem Summary-Tab lädt `App` über
`loadPaymentSummary` einmal je Datei in den `paymentSummary`-Store (dedupliziert per
Pfad). `SummaryView` und `RemittanceView` lesen denselben Store — kein Doppel-Parsen.
CodeViewer lädt seine XML unabhängig wie bisher.

## Fehler-/Leerfälle

- **XML-Tab:** unverändert (jetzt ohne Leer-Bug).
- **Kein Gläubiger:** Cdtr-Block wird nicht gerendert.
- **Keine PmtInf-Blöcke:** Übersicht zeigt „Keine Zahlungsblöcke in dieser Datei".
- **Keine Transaktionen:** Verwendungszweck zeigt „Keine Transaktionen in dieser Datei".
- **Alle Verwendungszwecke vorhanden:** kein Banner.
- **Nicht lesbar:** beide Summary-Tabs zeigen „Datei konnte nicht als XML gelesen
  werden" (Store-Status `error`).
- **Keine Datei gewählt:** neutraler Leerzustand.

## Test / Verifikation

- **Backend:** erweiterte Unit-Tests in `payments.rs` (LclInstrm/SeqTp, Creditor-Felder
  aus erstem PmtInf, Transaktion mit fehlendem und mit leerem Ustrd → `None`,
  Transaktions-Reihenfolge, pain.008-Fall, Nicht-Zahlungs-Dokument). `cd app/src-tauri
  && cargo test` grün.
- **Frontend:** `cd app && npm run check` grün.
- Manueller GUI-Check: Tab-Wechsel XML↔Übersicht↔Verwendungszweck ohne Leer-Bug;
  Cdtr-Block, neue Spalten, Banner + rote Markierung bei fehlendem Ustrd.

## Bewusst weggelassen (YAGNI)

- Adressblock/weitere Cdtr-Felder; Debtor-Block.
- Cdtr je PmtInf (nur erster Gläubiger).
- Export der Tabs; Caching über Dateiwechsel hinaus.
- Unterscheidung „leer" vs. „fehlt" in der Anzeige (beide = „kein Verwendungszweck").
