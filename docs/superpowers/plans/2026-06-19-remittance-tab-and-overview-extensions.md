# Remittance Tab, Overview Extensions & Viewer Bugfix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the per-file payment overview into an **Übersicht** tab (creditor block + PmtInf stats incl. LclInstrm/SeqTp) and a **Verwendungszweck** tab (per-transaction remittance list with empty/missing warnings), and fix the bug where switching Übersicht → XML leaves the XML viewer empty.

**Architecture:** Extend the read-only `payments.rs` quick-xml extractor (creditor, LclInstrm, SeqTp, per-transaction entries). On the frontend, a shared `paymentSummary` store loads once per file; two presentational tabs read it. The empty-XML bug is fixed by keeping `CodeViewer` permanently mounted and hiding it with CSS instead of unmounting it via `{#if}`.

**Tech Stack:** Rust + Tauri v2, `quick-xml`; Svelte 5 + TypeScript + Vite.

## Global Constraints

- Extraction stays read-only/on-demand; no change to validation, `ValidationResult`, `start_validation`, or the `read_payment_summary` command signature.
- Match LOCAL element names. Capture FIRST occurrence per field. PmtInf-level only for NbOfTxs/CtrlSum (not GrpHdr).
- LclInstrm = `PmtTpInf/LclInstrm/Cd`; SeqTp = `PmtTpInf/SeqTp`. SvcLvl/Cd stays `PmtTpInf/SvcLvl/Cd`.
- Creditor from the FIRST PmtInf only, and only at PmtInf level (NOT a transaction's Cdtr) — guard creditor fields with "not inside a transaction": name=`Cdtr/Nm`, iban=`CdtrAcct/Id/IBAN`, bic=`CdtrAgt/FinInstnId/BIC`|`BICFI`, creditorId=`CdtrSchmeId/Id/PrvtId/Othr/Id`.
- One `RemittanceEntry` per transaction element (`CdtTrfTxInf`/`DrctDbtTxInf`) in document order; `ustrd = None` when the transaction has no non-empty Ustrd (empty and missing both → `None`); multiple Ustrd joined with `"\n"`.
- Serde DTOs `#[serde(rename_all = "camelCase")]`; TS types mirror exactly (`Option<String>` → `string | null`).
- The flat `ustrd: Vec<String>` field is REPLACED by `transactions: Vec<RemittanceEntry>`; `creditor: Option<Creditor>` is added.
- Bugfix: `CodeViewer` must remain mounted across tab switches (hidden via CSS), never unmounted by `{#if}`.
- Tabs: `XML | Übersicht | Verwendungszweck`. Search/Collapse/Expand buttons only in the XML tab. Empty cells render `—`. Warning banner text: `⚠ N von M Transaktionen ohne Verwendungszweck`; missing entry text: `⚠ kein Verwendungszweck`.
- No JS test runner by design (frontend verification `npm run check`, 0 errors/0 warnings); backend `cargo test`.
- Commit format: `type(scope): summary`.

---

### Task 1: Extend the payments backend

Rewrite `payments.rs`: new DTOs (Creditor, LclInstrm/SeqTp, RemittanceEntry, transactions replacing ustrd, creditor), extended parser, updated + new tests. (This refactors existing tests — the `ustrd` assertions become `transactions`/`creditor` — so the test cycle is run once at the end, GREEN.)

**Files:**
- Modify (full rewrite): `app/src-tauri/src/payments.rs`

**Interfaces:**
- Consumes: `crate::validator::detect_namespace` (already `pub`).
- Produces (serde camelCase → JSON):
  - `Creditor { name, iban, bic, creditorId }` (all `Option<String>` → `string | null`).
  - `PmtInfSummary { nbOfTxs, ctrlSum, svcLvlCd, lclInstrm, seqTp, reqdDate }`.
  - `RemittanceEntry { ustrd: Option<String> }`.
  - `PaymentSummary { messageType, pmtInfCount, creditor: Option<Creditor>, blocks, transactions }`.
  - `extract_payment_summary(path: &Path) -> Result<PaymentSummary, String>` (unchanged signature).

- [ ] **Step 1: Replace the whole file**

Overwrite `app/src-tauri/src/payments.rs` with exactly:

```rust
//! Per-file SEPA payment summary: creditor + PmtInf block stats + per-transaction
//! remittance info. Read-only extraction with quick-xml; does not affect validation.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;

use crate::validator::detect_namespace;

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Creditor {
    pub name: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub creditor_id: Option<String>,
}

impl Creditor {
    fn has_any(&self) -> bool {
        self.name.is_some() || self.iban.is_some() || self.bic.is_some() || self.creditor_id.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PmtInfSummary {
    pub nb_of_txs: Option<String>,
    pub ctrl_sum: Option<String>,
    pub svc_lvl_cd: Option<String>,
    pub lcl_instrm: Option<String>,
    pub seq_tp: Option<String>,
    pub reqd_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemittanceEntry {
    pub ustrd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentSummary {
    pub message_type: String,
    pub pmt_inf_count: u32,
    pub creditor: Option<Creditor>,
    pub blocks: Vec<PmtInfSummary>,
    pub transactions: Vec<RemittanceEntry>,
}

/// Local element name (namespace prefix stripped) as an owned String.
fn local_of(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.into_owned(),
    }
}

pub fn extract_payment_summary(path: &Path) -> Result<PaymentSummary, String> {
    let message_type = detect_namespace(path)
        .map(|ns| ns.rsplit(':').next().unwrap_or("").to_string())
        .unwrap_or_default();

    let mut reader = Reader::from_file(path).map_err(|e| e.to_string())?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut blocks: Vec<PmtInfSummary> = Vec::new();
    let mut transactions: Vec<RemittanceEntry> = Vec::new();
    let mut current: Option<PmtInfSummary> = None;
    let mut current_creditor = Creditor::default();
    let mut creditor: Option<Creditor> = None;
    let mut current_tx: Option<Vec<String>> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let name = local_of(e.name().as_ref());
                match name.as_str() {
                    "PmtInf" => {
                        current = Some(PmtInfSummary::default());
                        current_creditor = Creditor::default();
                    }
                    "CdtTrfTxInf" | "DrctDbtTxInf" => current_tx = Some(Vec::new()),
                    _ => {}
                }
                stack.push(name);
            }
            Event::End(e) => {
                let name = local_of(e.name().as_ref());
                match name.as_str() {
                    "PmtInf" => {
                        if let Some(b) = current.take() {
                            blocks.push(b);
                        }
                        if creditor.is_none() && current_creditor.has_any() {
                            creditor = Some(std::mem::take(&mut current_creditor));
                        }
                    }
                    "CdtTrfTxInf" | "DrctDbtTxInf" => {
                        if let Some(tx) = current_tx.take() {
                            let ustrd = if tx.is_empty() { None } else { Some(tx.join("\n")) };
                            transactions.push(RemittanceEntry { ustrd });
                        }
                    }
                    _ => {}
                }
                stack.pop();
            }
            Event::Text(t) => {
                if current.is_none() {
                    continue;
                }
                let top = stack.last().map(String::as_str).unwrap_or("");
                let parent = stack.iter().rev().nth(1).map(String::as_str).unwrap_or("");
                let grand = stack.iter().rev().nth(2).map(String::as_str).unwrap_or("");
                let text = t.unescape().map_err(|e| e.to_string())?.into_owned();
                let b = current.as_mut().unwrap();
                match top {
                    "NbOfTxs" if parent == "PmtInf" => {
                        b.nb_of_txs.get_or_insert(text);
                    }
                    "CtrlSum" if parent == "PmtInf" => {
                        b.ctrl_sum.get_or_insert(text);
                    }
                    "Cd" if parent == "SvcLvl" && grand == "PmtTpInf" => {
                        b.svc_lvl_cd.get_or_insert(text);
                    }
                    "Cd" if parent == "LclInstrm" && grand == "PmtTpInf" => {
                        b.lcl_instrm.get_or_insert(text);
                    }
                    "SeqTp" if parent == "PmtTpInf" => {
                        b.seq_tp.get_or_insert(text);
                    }
                    "ReqdExctnDt" | "ReqdColltnDt" => {
                        b.reqd_date.get_or_insert(text);
                    }
                    "Dt" | "DtTm" if parent == "ReqdExctnDt" || parent == "ReqdColltnDt" => {
                        b.reqd_date.get_or_insert(text);
                    }
                    "Nm" if parent == "Cdtr" && current_tx.is_none() => {
                        current_creditor.name.get_or_insert(text);
                    }
                    "IBAN" if parent == "Id" && grand == "CdtrAcct" && current_tx.is_none() => {
                        current_creditor.iban.get_or_insert(text);
                    }
                    "BIC" | "BICFI"
                        if parent == "FinInstnId" && grand == "CdtrAgt" && current_tx.is_none() =>
                    {
                        current_creditor.bic.get_or_insert(text);
                    }
                    "Id" if parent == "Othr"
                        && current_tx.is_none()
                        && stack.iter().any(|s| s == "CdtrSchmeId") =>
                    {
                        current_creditor.creditor_id.get_or_insert(text);
                    }
                    "Ustrd" => {
                        if let Some(tx) = current_tx.as_mut() {
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                tx.push(trimmed.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    Ok(PaymentSummary {
        message_type,
        pmt_inf_count: blocks.len() as u32,
        creditor,
        blocks,
        transactions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_xml(name: &str, contents: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        p
    }

    const PAIN001: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.03">
  <CstmrCdtTrfInitn>
    <GrpHdr><MsgId>M1</MsgId><NbOfTxs>3</NbOfTxs><CtrlSum>600.00</CtrlSum></GrpHdr>
    <PmtInf>
      <PmtInfId>P1</PmtInfId>
      <NbOfTxs>2</NbOfTxs>
      <CtrlSum>300.00</CtrlSum>
      <PmtTpInf><SvcLvl><Cd>SEPA</Cd></SvcLvl></PmtTpInf>
      <ReqdExctnDt>2026-06-20</ReqdExctnDt>
      <CdtTrfTxInf><Cdtr><Nm>Payee One</Nm></Cdtr><RmtInf><Ustrd>Invoice 1</Ustrd></RmtInf></CdtTrfTxInf>
      <CdtTrfTxInf><Cdtr><Nm>Payee Two</Nm></Cdtr><RmtInf><Ustrd>Invoice 2</Ustrd></RmtInf></CdtTrfTxInf>
    </PmtInf>
    <PmtInf>
      <PmtInfId>P2</PmtInfId>
      <NbOfTxs>1</NbOfTxs>
      <CtrlSum>300.00</CtrlSum>
      <PmtTpInf><SvcLvl><Cd>SEPA</Cd></SvcLvl></PmtTpInf>
      <ReqdExctnDt>2026-06-21</ReqdExctnDt>
      <CdtTrfTxInf><RmtInf><Ustrd>Invoice 3</Ustrd></RmtInf></CdtTrfTxInf>
    </PmtInf>
  </CstmrCdtTrfInitn>
</Document>"#;

    #[test]
    fn pain001_blocks_transactions_and_no_pmtinf_creditor() {
        let p = temp_xml("sepa_sum2_p001.xml", PAIN001);
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.message_type, "pain.001.001.03");
        assert_eq!(s.pmt_inf_count, 2);
        // PmtInf-level values, not GrpHdr (3 / 600.00).
        assert_eq!(s.blocks[0].nb_of_txs.as_deref(), Some("2"));
        assert_eq!(s.blocks[0].ctrl_sum.as_deref(), Some("300.00"));
        assert_eq!(s.blocks[0].svc_lvl_cd.as_deref(), Some("SEPA"));
        assert_eq!(s.blocks[0].lcl_instrm, None);
        assert_eq!(s.blocks[0].seq_tp, None);
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-06-20"));
        // pain.001: Cdtr is per transaction, so there is NO PmtInf-level creditor.
        assert_eq!(s.creditor, None);
        // One entry per transaction, document order.
        assert_eq!(s.transactions.len(), 3);
        assert_eq!(s.transactions[0].ustrd.as_deref(), Some("Invoice 1"));
        assert_eq!(s.transactions[1].ustrd.as_deref(), Some("Invoice 2"));
        assert_eq!(s.transactions[2].ustrd.as_deref(), Some("Invoice 3"));
    }

    const PAIN008: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.02">
  <CstmrDrctDbtInitn>
    <GrpHdr><MsgId>M2</MsgId></GrpHdr>
    <PmtInf>
      <NbOfTxs>3</NbOfTxs>
      <CtrlSum>150.00</CtrlSum>
      <PmtTpInf>
        <SvcLvl><Cd>SEPA</Cd></SvcLvl>
        <LclInstrm><Cd>CORE</Cd></LclInstrm>
        <SeqTp>RCUR</SeqTp>
      </PmtTpInf>
      <ReqdColltnDt>2026-07-01</ReqdColltnDt>
      <Cdtr><Nm>ACME GmbH</Nm></Cdtr>
      <CdtrAcct><Id><IBAN>DE89370400440532013000</IBAN></Id></CdtrAcct>
      <CdtrAgt><FinInstnId><BIC>COBADEFFXXX</BIC></FinInstnId></CdtrAgt>
      <CdtrSchmeId><Id><PrvtId><Othr><Id>DE98ZZZ09999999999</Id></Othr></PrvtId></Id></CdtrSchmeId>
      <DrctDbtTxInf><RmtInf><Ustrd>Beitrag Mai</Ustrd></RmtInf></DrctDbtTxInf>
      <DrctDbtTxInf><RmtInf><Ustrd></Ustrd></RmtInf></DrctDbtTxInf>
      <DrctDbtTxInf></DrctDbtTxInf>
    </PmtInf>
  </CstmrDrctDbtInitn>
</Document>"#;

    #[test]
    fn pain008_creditor_lclinstrm_seqtp_and_missing_empty_ustrd() {
        let p = temp_xml("sepa_sum2_p008.xml", PAIN008);
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.message_type, "pain.008.001.02");
        assert_eq!(s.pmt_inf_count, 1);
        assert_eq!(s.blocks[0].lcl_instrm.as_deref(), Some("CORE"));
        assert_eq!(s.blocks[0].seq_tp.as_deref(), Some("RCUR"));
        assert_eq!(s.blocks[0].svc_lvl_cd.as_deref(), Some("SEPA"));
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-07-01"));
        let c = s.creditor.expect("creditor present");
        assert_eq!(c.name.as_deref(), Some("ACME GmbH"));
        assert_eq!(c.iban.as_deref(), Some("DE89370400440532013000"));
        assert_eq!(c.bic.as_deref(), Some("COBADEFFXXX"));
        assert_eq!(c.creditor_id.as_deref(), Some("DE98ZZZ09999999999"));
        assert_eq!(s.transactions.len(), 3);
        assert_eq!(s.transactions[0].ustrd.as_deref(), Some("Beitrag Mai"));
        assert_eq!(s.transactions[1].ustrd, None); // empty <Ustrd></Ustrd>
        assert_eq!(s.transactions[2].ustrd, None); // no RmtInf/Ustrd
    }

    #[test]
    fn nested_reqd_exctn_dt_resolves_inner_date() {
        let p = temp_xml(
            "sepa_sum2_p009.xml",
            r#"<?xml version="1.0"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09">
  <CstmrCdtTrfInitn>
    <GrpHdr><MsgId>M3</MsgId></GrpHdr>
    <PmtInf>
      <NbOfTxs>1</NbOfTxs>
      <PmtTpInf><SvcLvl><Cd>SEPA</Cd></SvcLvl></PmtTpInf>
      <ReqdExctnDt><Dt>2026-08-15</Dt></ReqdExctnDt>
      <CdtTrfTxInf><RmtInf><Ustrd>Nested date</Ustrd></RmtInf></CdtTrfTxInf>
    </PmtInf>
  </CstmrCdtTrfInitn>
</Document>"#,
        );
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-08-15"));
        assert_eq!(s.transactions.len(), 1);
        assert_eq!(s.transactions[0].ustrd.as_deref(), Some("Nested date"));
    }

    #[test]
    fn non_payment_doc_is_empty() {
        let p = temp_xml(
            "sepa_sum2_none.xml",
            r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.002.001.10"><CstmrPmtStsRpt><GrpHdr><MsgId>X</MsgId></GrpHdr></CstmrPmtStsRpt></Document>"#,
        );
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.pmt_inf_count, 0);
        assert!(s.blocks.is_empty());
        assert!(s.transactions.is_empty());
        assert_eq!(s.creditor, None);
    }

    #[test]
    fn malformed_is_err() {
        let p = temp_xml("sepa_sum2_bad.xml", "<a><b></a>");
        assert!(extract_payment_summary(&p).is_err());
    }
}
```

- [ ] **Step 2: Run the payments tests**

Run: `cd app/src-tauri && cargo test payments`
Expected: all 5 `payments` tests pass (GREEN).

- [ ] **Step 3: Full test run**

Run: `cd app/src-tauri && cargo test`
Expected: all tests pass (20 non-payments + 5 payments = 25), 0 failures, no warnings.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/payments.rs
git commit -m "feat(app): extract creditor, LclInstrm/SeqTp, per-transaction remittance"
```

---

### Task 2: Frontend types, shared store, Übersicht rebuild + viewer bugfix

Replace the payment-summary TS types, add a shared `paymentSummary` store, rebuild `SummaryView` (creditor block + extended stats table, reading the store), and update `App.svelte` to fix the empty-XML bug (keep `CodeViewer` mounted, hide via CSS) and drive the store loader. The XML/Übersicht two-tab structure is kept; the Verwendungszweck tab comes in Task 3.

**Files:**
- Modify: `app/src/lib/types.ts`
- Create: `app/src/lib/paymentSummary.ts`
- Modify (rewrite): `app/src/lib/SummaryView.svelte`
- Modify: `app/src/App.svelte`
- Modify: `app/src/app.css`

**Interfaces:**
- Consumes: `read_payment_summary` JSON (Task 1 shape); existing `readPaymentSummary` in `api.ts`; `selectedResult` store; existing viewer stores.
- Produces:
  - TS `Creditor`, `RemittanceEntry`, updated `PmtInfSummary` (+`lclInstrm`,`seqTp`), updated `PaymentSummary` (`creditor`, `transactions`, no `ustrd`).
  - `paymentSummary` writable `{ path: string; state: "idle"|"loading"|"ready"|"error"; data: PaymentSummary | null }` and `loadPaymentSummary(path: string | undefined): Promise<void>` (consumed by Task 3).
  - `.viewer-pane` wrapper + `.hidden` CSS (consumed by Task 3).

- [ ] **Step 1: Replace the payment-summary types**

In `app/src/lib/types.ts`, find the existing block (added by the previous feature):

```ts
export interface PmtInfSummary {
  nbOfTxs: string | null;
  ctrlSum: string | null;
  svcLvlCd: string | null;
  reqdDate: string | null;
}

export interface PaymentSummary {
  messageType: string;
  pmtInfCount: number;
  blocks: PmtInfSummary[];
  ustrd: string[];
}
```

and replace it with:

```ts
export interface Creditor {
  name: string | null;
  iban: string | null;
  bic: string | null;
  creditorId: string | null;
}

export interface PmtInfSummary {
  nbOfTxs: string | null;
  ctrlSum: string | null;
  svcLvlCd: string | null;
  lclInstrm: string | null;
  seqTp: string | null;
  reqdDate: string | null;
}

export interface RemittanceEntry {
  ustrd: string | null;
}

export interface PaymentSummary {
  messageType: string;
  pmtInfCount: number;
  creditor: Creditor | null;
  blocks: PmtInfSummary[];
  transactions: RemittanceEntry[];
}
```

- [ ] **Step 2: Create the shared store**

Create `app/src/lib/paymentSummary.ts`:

```ts
import { writable, get } from "svelte/store";
import { readPaymentSummary } from "./api";
import type { PaymentSummary } from "./types";

export type SummaryState = "idle" | "loading" | "ready" | "error";

export const paymentSummary = writable<{
  path: string;
  state: SummaryState;
  data: PaymentSummary | null;
}>({ path: "", state: "idle", data: null });

/** Load the summary for `path` into the store, deduped by path. */
export async function loadPaymentSummary(path: string | undefined): Promise<void> {
  if (!path) {
    paymentSummary.set({ path: "", state: "idle", data: null });
    return;
  }
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

- [ ] **Step 3: Rebuild SummaryView (stats only, store-based)**

Overwrite `app/src/lib/SummaryView.svelte` with:

```svelte
<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  $: ps = $paymentSummary;
</script>

<div class="summary">
  {#if !$selectedResult}
    <p class="muted">Keine Datei ausgewählt.</p>
  {:else if ps.state === "error"}
    <p class="muted">Datei konnte nicht als XML gelesen werden.</p>
  {:else if ps.state === "ready" && ps.data}
    {@const data = ps.data}
    {#if data.creditor}
      <h3>Gläubiger (Cdtr)</h3>
      <dl class="cdtr">
        <dt>Name</dt><dd>{data.creditor.name ?? "—"}</dd>
        <dt>IBAN</dt><dd>{data.creditor.iban ?? "—"}</dd>
        <dt>BIC</dt><dd>{data.creditor.bic ?? "—"}</dd>
        <dt>Gläubiger-ID</dt><dd>{data.creditor.creditorId ?? "—"}</dd>
      </dl>
    {/if}
    <h3>
      {data.pmtInfCount} PmtInf-{data.pmtInfCount === 1 ? "Block" : "Blöcke"}{data.messageType
        ? ` · ${data.messageType}`
        : ""}
    </h3>
    {#if data.blocks.length === 0}
      <p class="muted">Keine Zahlungsblöcke in dieser Datei.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>#</th><th>NbOfTxs</th><th>CtrlSum</th><th>SvcLvl/Cd</th>
            <th>LclInstrm</th><th>SeqTp</th><th>Datum</th>
          </tr>
        </thead>
        <tbody>
          {#each data.blocks as b, i}
            <tr>
              <td>{i + 1}</td>
              <td>{b.nbOfTxs ?? "—"}</td>
              <td>{b.ctrlSum ?? "—"}</td>
              <td>{b.svcLvlCd ?? "—"}</td>
              <td>{b.lclInstrm ?? "—"}</td>
              <td>{b.seqTp ?? "—"}</td>
              <td>{b.reqdDate ?? "—"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {:else}
    <p class="muted">Lädt…</p>
  {/if}
</div>

<style>
  .summary { padding: 10px 14px; }
  .summary h3 { font-size: 13px; margin: 14px 0 6px; }
  .summary h3:first-child { margin-top: 0; }
  .muted { opacity: 0.7; font-style: italic; }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--border); }
  th { font-weight: 600; }
  dl.cdtr { display: grid; grid-template-columns: max-content 1fr; gap: 2px 12px; margin: 0 0 6px; font-size: 12px; }
  dl.cdtr dt { font-weight: 600; }
  dl.cdtr dd { margin: 0; word-break: break-word; }
</style>
```

- [ ] **Step 4: Fix the bug and drive the loader in App.svelte**

In `app/src/App.svelte`, add an import after the existing store import line (`import { selectedResult, openViewerSearch, foldAllInViewer, unfoldAllInViewer } from "./lib/stores";`):

```ts
  import { loadPaymentSummary } from "./lib/paymentSummary";
```

Then after the `let viewerTab: "xml" | "summary" = "xml";` line, add the reactive loader:

```ts
  $: if (viewerTab !== "xml") loadPaymentSummary($selectedResult?.path);
```

Now replace the entire viewer `<section>` (currently from `<section class="viewer">` through its closing `</section>`, the block that contains the `{#if viewerTab === "xml"}<CodeViewer />{:else}<SummaryView />{/if}`) with:

```svelte
    <section class="viewer">
      <div class="viewer-bar">
        <div class="viewer-tabs">
          <button class:active={viewerTab === "xml"} on:click={() => (viewerTab = "xml")}>XML</button>
          <button class:active={viewerTab === "summary"} on:click={() => (viewerTab = "summary")}>Übersicht</button>
        </div>
        {#if viewerTab === "xml"}
          <button on:click={() => $openViewerSearch()} disabled={!$selectedResult}>Search</button>
          <button on:click={() => $foldAllInViewer()} disabled={!$selectedResult}>Collapse all</button>
          <button on:click={() => $unfoldAllInViewer()} disabled={!$selectedResult}>Expand all</button>
        {/if}
      </div>
      <div class="viewer-pane" class:hidden={viewerTab !== "xml"}><CodeViewer /></div>
      {#if viewerTab === "summary"}<SummaryView />{/if}
    </section>
```

- [ ] **Step 5: Add the viewer-pane CSS**

In `app/src/app.css`, find the line:

```css
.summary { flex: 1 1 auto; min-height: 0; overflow: auto; }
```

and insert directly after it:

```css
.viewer-pane { flex: 1 1 auto; min-height: 0; display: flex; flex-direction: column; }
.viewer-pane.hidden { display: none; }
```

- [ ] **Step 6: Type-check**

Run: `cd app && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 7: Commit**

```bash
git add app/src/lib/types.ts app/src/lib/paymentSummary.ts app/src/lib/SummaryView.svelte app/src/App.svelte app/src/app.css
git commit -m "feat(app): creditor + LclInstrm/SeqTp in overview; keep XML viewer mounted (bugfix)"
```

---

### Task 3: Verwendungszweck tab with per-transaction warnings

Add `RemittanceView.svelte` (per-transaction list + warning banner + missing markers) and wire the third tab into `App.svelte`; small CSS for the 3-tab border.

**Files:**
- Create: `app/src/lib/RemittanceView.svelte`
- Modify: `app/src/App.svelte`
- Modify: `app/src/app.css`

**Interfaces:**
- Consumes: `paymentSummary` store (Task 2); `selectedResult` store; the `.summary` fill rule and `.viewer-tabs` markup from Task 2.
- Produces: nothing for later tasks (final task).

- [ ] **Step 1: Create RemittanceView.svelte**

Create `app/src/lib/RemittanceView.svelte`:

```svelte
<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  $: ps = $paymentSummary;
</script>

<div class="summary">
  {#if !$selectedResult}
    <p class="muted">Keine Datei ausgewählt.</p>
  {:else if ps.state === "error"}
    <p class="muted">Datei konnte nicht als XML gelesen werden.</p>
  {:else if ps.state === "ready" && ps.data}
    {@const data = ps.data}
    {@const total = data.transactions.length}
    {@const missing = data.transactions.filter((t) => t.ustrd == null).length}
    {#if total === 0}
      <p class="muted">Keine Transaktionen in dieser Datei.</p>
    {:else}
      {#if missing > 0}
        <p class="warn-banner">⚠ {missing} von {total} Transaktionen ohne Verwendungszweck</p>
      {/if}
      <ol class="ustrd">
        {#each data.transactions as t}
          {#if t.ustrd == null}
            <li class="missing">⚠ kein Verwendungszweck</li>
          {:else}
            <li>{t.ustrd}</li>
          {/if}
        {/each}
      </ol>
    {/if}
  {:else}
    <p class="muted">Lädt…</p>
  {/if}
</div>

<style>
  .summary { padding: 10px 14px; }
  .muted { opacity: 0.7; font-style: italic; }
  .warn-banner {
    background: rgba(196, 39, 28, 0.12);
    color: var(--err);
    border: 1px solid var(--err);
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 12px;
    margin: 0 0 10px;
  }
  ol.ustrd { margin: 0; padding-left: 22px; font-size: 13px; }
  ol.ustrd li { padding: 2px 0; word-break: break-word; white-space: pre-wrap; }
  ol.ustrd li.missing { color: var(--err); font-style: italic; }
</style>
```

- [ ] **Step 2: Wire the third tab into App.svelte**

In `app/src/App.svelte`, add the import after `import SummaryView from "./lib/SummaryView.svelte";`:

```ts
  import RemittanceView from "./lib/RemittanceView.svelte";
```

Widen the tab type — change:

```ts
  let viewerTab: "xml" | "summary" = "xml";
```

to:

```ts
  let viewerTab: "xml" | "summary" | "remittance" = "xml";
```

In the markup, add the third tab button directly after the Übersicht button:

```svelte
          <button class:active={viewerTab === "summary"} on:click={() => (viewerTab = "summary")}>Übersicht</button>
          <button class:active={viewerTab === "remittance"} on:click={() => (viewerTab = "remittance")}>Verwendungszweck</button>
```

And add the RemittanceView mount directly after the SummaryView mount line:

```svelte
      {#if viewerTab === "summary"}<SummaryView />{/if}
      {#if viewerTab === "remittance"}<RemittanceView />{/if}
```

- [ ] **Step 3: Fix the tab border for three buttons**

In `app/src/app.css`, find these two lines:

```css
.viewer-bar .viewer-tabs button:first-child { border-radius: 6px 0 0 6px; }
.viewer-bar .viewer-tabs button:last-child { border-radius: 0 6px 6px 0; border-left: none; }
```

and replace them with:

```css
.viewer-bar .viewer-tabs button:not(:first-child) { border-left: none; }
.viewer-bar .viewer-tabs button:first-child { border-radius: 6px 0 0 6px; }
.viewer-bar .viewer-tabs button:last-child { border-radius: 0 6px 6px 0; }
```

- [ ] **Step 4: Type-check**

Run: `cd app && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/RemittanceView.svelte app/src/App.svelte app/src/app.css
git commit -m "feat(app): Verwendungszweck tab with per-transaction empty/missing warnings"
```

---

## Self-Review

**Spec coverage:**
- Bugfix (empty XML after tab switch) → Task 2 Step 4 (CodeViewer wrapped, `class:hidden`, never unmounted) + Step 5 CSS.
- Tabs XML/Übersicht/Verwendungszweck → Task 2 (XML/Übersicht) + Task 3 (Verwendungszweck).
- Übersicht: Cdtr block (Name/IBAN/BIC/Gläubiger-ID, first PmtInf, hidden if none) → Task 1 parser + Task 2 SummaryView `{#if data.creditor}`.
- Übersicht: LclInstrm + SeqTp columns → Task 1 parser + Task 2 table headers/cells.
- Verwendungszweck: per-transaction list, banner `N von M`, red `⚠ kein Verwendungszweck` → Task 1 `transactions` + Task 3 RemittanceView.
- Empty AND missing Ustrd both → `None`/warning → Task 1 (empty Ustrd yields no text → tx empty → None; missing RmtInf → None) + tests `pain008_…`.
- Creditor only from PmtInf level, not transaction Cdtr → Task 1 `current_tx.is_none()` guards, verified by `pain001_…` asserting `creditor == None`.
- Shared single fetch per file → Task 2 `paymentSummary` store + `loadPaymentSummary` deduped by path; App drives it.
- Empty/error states → Task 1 (empty vecs / Err) + Tasks 2/3 conditional blocks.
- DTO camelCase ↔ TS → Task 1 serde + Task 2 types.
- YAGNI exclusions (address, debtor, export, caching beyond file, leer/fehlt distinction) → not implemented. ✓

**Placeholder scan:** No TBD/TODO; every code step has complete code. ✓

**Type consistency:** Rust fields `message_type/pmt_inf_count/creditor/blocks/transactions` and `name/iban/bic/creditor_id`, `nb_of_txs/ctrl_sum/svc_lvl_cd/lcl_instrm/seq_tp/reqd_date`, `ustrd` serialize camelCase → `messageType/pmtInfCount/creditor/blocks/transactions`, `name/iban/bic/creditorId`, `nbOfTxs/ctrlSum/svcLvlCd/lclInstrm/seqTp/reqdDate`, `ustrd` — matching the Task 2 TS interfaces and the template usages in Tasks 2/3 (`data.creditor.creditorId`, `b.lclInstrm`, `b.seqTp`, `data.transactions`, `t.ustrd`). Store name `paymentSummary` and `loadPaymentSummary` match between `paymentSummary.ts`, `App.svelte`, `SummaryView.svelte`, `RemittanceView.svelte`. Command/function names unchanged from prior feature. ✓
