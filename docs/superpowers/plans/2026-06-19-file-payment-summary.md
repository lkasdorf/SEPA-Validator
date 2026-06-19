# Per-File Payment Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show, per selected file, a flat list of all `Ustrd` remittance-info values and a small `PmtInf` statistics table (NbOfTxs, CtrlSum, SvcLvl/Cd, execution/collection date) in a new "Übersicht" tab of the XML viewer.

**Architecture:** A new read-only Rust module (`payments.rs`) extracts the data with `quick-xml` (matching local element names), exposed via a new Tauri command `read_payment_summary`. The Svelte frontend gains a `viewerTab` toggle ("XML | Übersicht") in the existing viewer bar and a `SummaryView.svelte` that renders the data on demand. Validation is untouched.

**Tech Stack:** Rust + Tauri v2, `quick-xml` (already a dependency); Svelte 5 + TypeScript + Vite.

## Global Constraints

- Extraction is read-only and on-demand; it must NOT change validation, `ValidationResult`, or any existing command.
- Match **local** element names (ignore namespace prefix). Read `NbOfTxs`/`CtrlSum` only at `PmtInf` level (NOT `GrpHdr`); read `Cd` only under `PmtTpInf/SvcLvl`; read `Ustrd` only inside `PmtInf`. Capture the **first** occurrence per field, in document order for `ustrd`.
- Date field is `ReqdExctnDt` (pain.001) OR `ReqdColltnDt` (pain.008); if the date element wraps `Dt`/`DtTm` (pain.x.09), use the inner text.
- All extracted numbers/dates are kept as text (`Option<String>`); no parsing/rounding.
- Serde DTOs use `#[serde(rename_all = "camelCase")]`; the TypeScript types must mirror them exactly (`Option<String>` → `string | null`).
- UI labels are the German/English mix already in the app; new tab labels: `XML` and `Übersicht`. Empty values render as `—`.
- Reuse the existing quick-xml style from `formatting.rs` (`Reader::from_file`, `reader.config_mut().trim_text(true)`, `read_event_into`).
- No frontend test runner exists by design; frontend verification is `cd app && npm run check` (0 errors/0 warnings). Backend verification is `cd app/src-tauri && cargo test`.
- Commit format: `type(scope): summary` (e.g. `feat(app): ...`).

---

### Task 1: Backend payments module + command

Create the extraction module with DTOs, tests, the real parser, and the Tauri command. TDD: write DTOs + a stub + tests (RED), then implement the parser (GREEN), then wire the command.

**Files:**
- Create: `app/src-tauri/src/payments.rs`
- Modify: `app/src-tauri/src/lib.rs` (register `mod payments;` and the new command in `generate_handler!`)
- Modify: `app/src-tauri/src/commands.rs` (add `read_payment_summary`)

**Interfaces:**
- Consumes: `crate::validator::detect_namespace(path: &Path) -> Option<String>` (already `pub`).
- Produces:
  - `payments::PaymentSummary { message_type: String, pmt_inf_count: u32, blocks: Vec<PaymentSummary's PmtInfSummary>, ustrd: Vec<String> }` (serde camelCase → `messageType`, `pmtInfCount`, `blocks`, `ustrd`).
  - `payments::PmtInfSummary { nb_of_txs: Option<String>, ctrl_sum: Option<String>, svc_lvl_cd: Option<String>, reqd_date: Option<String> }` (camelCase → `nbOfTxs`, `ctrlSum`, `svcLvlCd`, `reqdDate`).
  - `payments::extract_payment_summary(path: &Path) -> Result<PaymentSummary, String>`.
  - Tauri command `read_payment_summary(path: String) -> Result<PaymentSummary, String>`.

- [ ] **Step 1: Create the module with DTOs, a stub, and tests**

Create `app/src-tauri/src/payments.rs`:

```rust
//! Per-file SEPA payment summary: PmtInf block stats + flat Ustrd list.
//! Read-only extraction with quick-xml; does not affect validation.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;

use crate::validator::detect_namespace;

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PmtInfSummary {
    pub nb_of_txs: Option<String>,
    pub ctrl_sum: Option<String>,
    pub svc_lvl_cd: Option<String>,
    pub reqd_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentSummary {
    pub message_type: String,
    pub pmt_inf_count: u32,
    pub blocks: Vec<PmtInfSummary>,
    pub ustrd: Vec<String>,
}

/// Local element name (namespace prefix stripped) as an owned String.
fn local_of(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.into_owned(),
    }
}

pub fn extract_payment_summary(_path: &Path) -> Result<PaymentSummary, String> {
    // Stub — replaced in Step 3.
    Ok(PaymentSummary {
        message_type: String::new(),
        pmt_inf_count: 0,
        blocks: Vec::new(),
        ustrd: Vec::new(),
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
      <CdtTrfTxInf><RmtInf><Ustrd>Invoice 1</Ustrd></RmtInf></CdtTrfTxInf>
      <CdtTrfTxInf><RmtInf><Ustrd>Invoice 2</Ustrd></RmtInf></CdtTrfTxInf>
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

    const PAIN008: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.02">
  <CstmrDrctDbtInitn>
    <GrpHdr><MsgId>M2</MsgId></GrpHdr>
    <PmtInf>
      <NbOfTxs>1</NbOfTxs>
      <CtrlSum>50.00</CtrlSum>
      <PmtTpInf><SvcLvl><Cd>SEPA</Cd></SvcLvl></PmtTpInf>
      <ReqdColltnDt>2026-07-01</ReqdColltnDt>
      <DrctDbtTxInf><RmtInf><Ustrd>Membership</Ustrd></RmtInf></DrctDbtTxInf>
    </PmtInf>
  </CstmrDrctDbtInitn>
</Document>"#;

    const PAIN001_09_NESTED: &str = r#"<?xml version="1.0"?>
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
</Document>"#;

    #[test]
    fn pain001_extracts_blocks_and_ustrd_in_order() {
        let p = temp_xml("sepa_sum_p001.xml", PAIN001);
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.message_type, "pain.001.001.03");
        assert_eq!(s.pmt_inf_count, 2);
        assert_eq!(s.blocks.len(), 2);
        // First block is PmtInf-level (2 / 300.00), NOT the GrpHdr (3 / 600.00).
        assert_eq!(s.blocks[0].nb_of_txs.as_deref(), Some("2"));
        assert_eq!(s.blocks[0].ctrl_sum.as_deref(), Some("300.00"));
        assert_eq!(s.blocks[0].svc_lvl_cd.as_deref(), Some("SEPA"));
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-06-20"));
        assert_eq!(s.blocks[1].reqd_date.as_deref(), Some("2026-06-21"));
        assert_eq!(s.ustrd, vec!["Invoice 1", "Invoice 2", "Invoice 3"]);
    }

    #[test]
    fn pain008_uses_collection_date() {
        let p = temp_xml("sepa_sum_p008.xml", PAIN008);
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.message_type, "pain.008.001.02");
        assert_eq!(s.pmt_inf_count, 1);
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-07-01"));
        assert_eq!(s.blocks[0].svc_lvl_cd.as_deref(), Some("SEPA"));
        assert_eq!(s.ustrd, vec!["Membership"]);
    }

    #[test]
    fn nested_reqd_exctn_dt_resolves_inner_date() {
        let p = temp_xml("sepa_sum_p009.xml", PAIN001_09_NESTED);
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-08-15"));
    }

    #[test]
    fn non_payment_doc_has_no_blocks() {
        let p = temp_xml(
            "sepa_sum_none.xml",
            r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.002.001.10"><CstmrPmtStsRpt><GrpHdr><MsgId>X</MsgId></GrpHdr></CstmrPmtStsRpt></Document>"#,
        );
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.pmt_inf_count, 0);
        assert!(s.blocks.is_empty());
        assert!(s.ustrd.is_empty());
    }

    #[test]
    fn malformed_is_err() {
        let p = temp_xml("sepa_sum_bad.xml", "<a><b></a>");
        assert!(extract_payment_summary(&p).is_err());
    }
}
```

Then register the module: in `app/src-tauri/src/lib.rs`, the module list is:

```rust
mod commands;
mod formatting;
mod model;
mod scanner;
mod schema;
mod validator;
```

Add `mod payments;` so the list reads:

```rust
mod commands;
mod formatting;
mod model;
mod payments;
mod scanner;
mod schema;
mod validator;
```

- [ ] **Step 2: Run the tests — expect RED**

Run: `cd app/src-tauri && cargo test payments`
Expected: compiles, but the extraction tests FAIL (the stub returns an empty summary, so `pmt_inf_count`/`blocks`/`ustrd` assertions fail and `malformed_is_err` fails because the stub never reads the file).

- [ ] **Step 3: Implement the real parser**

In `app/src-tauri/src/payments.rs`, replace the stub `extract_payment_summary` with:

```rust
pub fn extract_payment_summary(path: &Path) -> Result<PaymentSummary, String> {
    let message_type = detect_namespace(path)
        .map(|ns| ns.rsplit(':').next().unwrap_or("").to_string())
        .unwrap_or_default();

    let mut reader = Reader::from_file(path).map_err(|e| e.to_string())?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut blocks: Vec<PmtInfSummary> = Vec::new();
    let mut ustrd: Vec<String> = Vec::new();
    let mut current: Option<PmtInfSummary> = None;

    loop {
        match reader.read_event_into(&mut buf).map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let name = local_of(e.name().as_ref());
                if name == "PmtInf" {
                    current = Some(PmtInfSummary::default());
                }
                stack.push(name);
            }
            Event::End(e) => {
                let name = local_of(e.name().as_ref());
                if name == "PmtInf" {
                    if let Some(b) = current.take() {
                        blocks.push(b);
                    }
                }
                stack.pop();
            }
            Event::Text(t) => {
                let in_pmt_inf = stack.iter().any(|s| s == "PmtInf");
                if !in_pmt_inf {
                    buf.clear();
                    continue;
                }
                if let Some(b) = current.as_mut() {
                    let top = stack.last().map(String::as_str).unwrap_or("");
                    let parent = stack.iter().rev().nth(1).map(String::as_str).unwrap_or("");
                    let grand = stack.iter().rev().nth(2).map(String::as_str).unwrap_or("");
                    let text = t.unescape().map_err(|e| e.to_string())?.into_owned();
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
                        "ReqdExctnDt" | "ReqdColltnDt" => {
                            b.reqd_date.get_or_insert(text);
                        }
                        "Dt" | "DtTm" if parent == "ReqdExctnDt" || parent == "ReqdColltnDt" => {
                            b.reqd_date.get_or_insert(text);
                        }
                        "Ustrd" => ustrd.push(text),
                        _ => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(PaymentSummary {
        message_type,
        pmt_inf_count: blocks.len() as u32,
        blocks,
        ustrd,
    })
}
```

- [ ] **Step 4: Run the tests — expect GREEN**

Run: `cd app/src-tauri && cargo test payments`
Expected: all 5 `payments` tests pass.

- [ ] **Step 5: Add the Tauri command and register it**

In `app/src-tauri/src/commands.rs`, append after the existing `read_formatted` command:

```rust
/// Extract the per-file SEPA payment summary (PmtInf stats + Ustrd list).
#[tauri::command]
pub fn read_payment_summary(path: String) -> Result<crate::payments::PaymentSummary, String> {
    crate::payments::extract_payment_summary(std::path::Path::new(&path))
}
```

In `app/src-tauri/src/lib.rs`, the handler list currently is:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::start_validation,
            commands::read_file,
            commands::write_text_file,
            commands::read_formatted
        ])
```

Add the new command (note the comma after `read_formatted`):

```rust
        .invoke_handler(tauri::generate_handler![
            commands::start_validation,
            commands::read_file,
            commands::write_text_file,
            commands::read_formatted,
            commands::read_payment_summary
        ])
```

- [ ] **Step 6: Build + full test run**

Run: `cd app/src-tauri && cargo test`
Expected: all tests pass (5 new `payments` tests + the 18 unit + 2 spike already present), 0 failures, no warnings.

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/src/payments.rs app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs
git commit -m "feat(app): backend payment summary extraction + command"
```

---

### Task 2: Frontend data layer (types + api wrapper)

Add the TypeScript types mirroring the serde DTOs and the `invoke` wrapper.

**Files:**
- Modify: `app/src/lib/types.ts`
- Modify: `app/src/lib/api.ts`

**Interfaces:**
- Consumes: the Tauri command `read_payment_summary` from Task 1 (returns the camelCase JSON of `PaymentSummary`).
- Produces:
  - TS `PaymentSummary { messageType: string; pmtInfCount: number; blocks: PmtInfSummary[]; ustrd: string[] }`
  - TS `PmtInfSummary { nbOfTxs: string | null; ctrlSum: string | null; svcLvlCd: string | null; reqdDate: string | null }`
  - `readPaymentSummary(path: string): Promise<PaymentSummary>` (consumed by Task 3).

- [ ] **Step 1: Add the types**

In `app/src/lib/types.ts`, append at the end of the file (after `statusLabel`):

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

- [ ] **Step 2: Add the api wrapper**

In `app/src/lib/api.ts`, change the import line:

```ts
import type { ValidationEvent, ValidationResult } from "./types";
```

to:

```ts
import type { ValidationEvent, ValidationResult, PaymentSummary } from "./types";
```

Then add, after the existing `readFormatted` function:

```ts
export function readPaymentSummary(path: string): Promise<PaymentSummary> {
  return invoke<PaymentSummary>("read_payment_summary", { path });
}
```

- [ ] **Step 3: Type-check**

Run: `cd app && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Commit**

```bash
git add app/src/lib/types.ts app/src/lib/api.ts
git commit -m "feat(app): payment summary types + api wrapper"
```

---

### Task 3: Frontend UI (Übersicht tab + SummaryView)

Add the `viewerTab` toggle to the viewer bar and a `SummaryView.svelte` that renders the summary for the selected file.

**Files:**
- Create: `app/src/lib/SummaryView.svelte`
- Modify: `app/src/App.svelte` (tab toggle, conditional buttons, conditional view)
- Modify: `app/src/app.css` (tab + summary-fill styles)

**Interfaces:**
- Consumes: `readPaymentSummary` and `PaymentSummary` from Task 2; the existing `selectedResult` store; the existing `.viewer` flex-column layout and `openViewerSearch`/`foldAllInViewer`/`unfoldAllInViewer` stores.
- Produces: nothing for later tasks (final task).

- [ ] **Step 1: Create SummaryView.svelte**

Create `app/src/lib/SummaryView.svelte`:

```svelte
<script lang="ts">
  import { selectedResult } from "./stores";
  import { readPaymentSummary } from "./api";
  import type { PaymentSummary } from "./types";

  let summary: PaymentSummary | null = null;
  let error = "";
  let loadedPath = "";

  $: void load($selectedResult?.path);

  async function load(path: string | undefined) {
    if (!path) {
      summary = null;
      error = "";
      loadedPath = "";
      return;
    }
    if (path === loadedPath) return;
    loadedPath = path;
    error = "";
    summary = null;
    try {
      summary = await readPaymentSummary(path);
    } catch {
      summary = null;
      error = "Datei konnte nicht als XML gelesen werden.";
    }
  }
</script>

<div class="summary">
  {#if !$selectedResult}
    <p class="muted">Keine Datei ausgewählt.</p>
  {:else if error}
    <p class="muted">{error}</p>
  {:else if !summary}
    <p class="muted">Lädt…</p>
  {:else}
    <h3>
      {summary.pmtInfCount} PmtInf-{summary.pmtInfCount === 1 ? "Block" : "Blöcke"}{summary.messageType
        ? ` · ${summary.messageType}`
        : ""}
    </h3>
    {#if summary.blocks.length === 0}
      <p class="muted">Keine Zahlungsblöcke in dieser Datei.</p>
    {:else}
      <table>
        <thead>
          <tr><th>#</th><th>NbOfTxs</th><th>CtrlSum</th><th>SvcLvl/Cd</th><th>Datum</th></tr>
        </thead>
        <tbody>
          {#each summary.blocks as b, i}
            <tr>
              <td>{i + 1}</td>
              <td>{b.nbOfTxs ?? "—"}</td>
              <td>{b.ctrlSum ?? "—"}</td>
              <td>{b.svcLvlCd ?? "—"}</td>
              <td>{b.reqdDate ?? "—"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    <h3>Verwendungszwecke (Ustrd)</h3>
    {#if summary.ustrd.length === 0}
      <p class="muted">Keine Verwendungszwecke.</p>
    {:else}
      <ol class="ustrd">
        {#each summary.ustrd as u}
          <li>{u}</li>
        {/each}
      </ol>
    {/if}
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
  ol.ustrd { margin: 0; padding-left: 22px; font-size: 13px; }
  ol.ustrd li { padding: 2px 0; word-break: break-word; }
</style>
```

- [ ] **Step 2: Wire the tab toggle into App.svelte (script)**

In `app/src/App.svelte`, the component imports currently include:

```ts
  import CodeViewer from "./lib/CodeViewer.svelte";
```

Add a `SummaryView` import directly below it:

```ts
  import CodeViewer from "./lib/CodeViewer.svelte";
  import SummaryView from "./lib/SummaryView.svelte";
```

Then, after the existing store import line (`import { selectedResult, openViewerSearch, foldAllInViewer, unfoldAllInViewer } from "./lib/stores";`), add the local tab state:

```ts
  let viewerTab: "xml" | "summary" = "xml";
```

- [ ] **Step 3: Wire the tab toggle into App.svelte (markup)**

In `app/src/App.svelte`, the viewer section currently is:

```svelte
    <section class="viewer">
      <div class="viewer-bar">
        <button on:click={() => $openViewerSearch()} disabled={!$selectedResult}>Search</button>
        <button on:click={() => $foldAllInViewer()} disabled={!$selectedResult}>Collapse all</button>
        <button on:click={() => $unfoldAllInViewer()} disabled={!$selectedResult}>Expand all</button>
      </div>
      <CodeViewer />
    </section>
```

Replace it with:

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
      {#if viewerTab === "xml"}
        <CodeViewer />
      {:else}
        <SummaryView />
      {/if}
    </section>
```

- [ ] **Step 4: Add tab + summary-fill styles**

In `app/src/app.css`, the viewer block added by the previous feature is:

```css
.viewer { display: flex; flex-direction: column; }
.viewer-bar { display: flex; gap: 8px; padding: 6px 8px; border-bottom: 1px solid var(--border); flex: 0 0 auto; }
.viewer-bar button { background: var(--accent); color: #fff; border: none; padding: 4px 10px; border-radius: 6px; cursor: pointer; font-size: 12px; }
.viewer-bar button:hover:not(:disabled) { filter: brightness(1.1); }
.viewer-bar button:disabled { opacity: 0.45; cursor: default; }
```

Insert the following directly after that block:

```css
.viewer-bar .viewer-tabs { display: flex; gap: 0; margin-right: 6px; }
.viewer-bar .viewer-tabs button { background: transparent; color: var(--fg); border: 1px solid var(--border); border-radius: 0; }
.viewer-bar .viewer-tabs button:first-child { border-radius: 6px 0 0 6px; }
.viewer-bar .viewer-tabs button:last-child { border-radius: 0 6px 6px 0; border-left: none; }
.viewer-bar .viewer-tabs button.active { background: var(--accent); color: #fff; }
.summary { flex: 1 1 auto; min-height: 0; overflow: auto; }
```

- [ ] **Step 5: Type-check**

Run: `cd app && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/SummaryView.svelte app/src/App.svelte app/src/app.css
git commit -m "feat(app): Übersicht tab with per-file Ustrd list and PmtInf stats"
```

---

## Self-Review

**Spec coverage:**
- Ustrd list, flat, document order → Task 1 (`ustrd` vec + ordering test) + Task 3 (`<ol>`).
- PmtInf stats table (count + per-block NbOfTxs/CtrlSum/SvcLvl·Cd/date) → Task 1 (`blocks`, `pmt_inf_count`) + Task 3 (`<table>`).
- pain.001 (`ReqdExctnDt`) + pain.008 (`ReqdColltnDt`), generic local-name match, nested `Dt`/`DtTm` → Task 1 parser + tests `pain001…`, `pain008_uses_collection_date`, `nested_reqd_exctn_dt_resolves_inner_date`.
- GrpHdr NbOfTxs/CtrlSum excluded; `Cd` only under `PmtTpInf/SvcLvl`; `Ustrd` only in `PmtInf` → Task 1 parent/grand guards, verified by `pain001…` (block[0].nbOfTxs == "2", not GrpHdr "3").
- On-demand command, validation untouched → Task 1 `read_payment_summary` (separate command; no change to `start_validation`/`ValidationResult`).
- Tab "XML | Übersicht"; Search/Collapse/Expand only in XML tab; `{#if}` view switch → Task 3.
- Empty/error states (malformed, no PmtInf, no Ustrd, no file) → Task 1 `malformed_is_err`/`non_payment_doc_has_no_blocks` + Task 3 conditional blocks.
- DTO camelCase mirrored in TS → Task 1 serde attrs + Task 2 types.
- YAGNI exclusions (export, GrpHdr totals, Strd, Prtry, edit/sort/filter, caching) → not implemented. ✓

**Placeholder scan:** No TBD/TODO; every code step has complete code. ✓

**Type consistency:** Rust `PaymentSummary`/`PmtInfSummary` field names (`message_type`/`pmt_inf_count`/`nb_of_txs`/`ctrl_sum`/`svc_lvl_cd`/`reqd_date`) serialize via camelCase to `messageType`/`pmtInfCount`/`nbOfTxs`/`ctrlSum`/`svcLvlCd`/`reqdDate`, which match the TS interfaces (Task 2) and the template usage `b.nbOfTxs`/`b.ctrlSum`/`b.svcLvlCd`/`b.reqdDate`, `summary.pmtInfCount`/`summary.messageType`/`summary.blocks`/`summary.ustrd` (Task 3). Command name `read_payment_summary` matches across `lib.rs`, `commands.rs`, and `api.ts` `invoke("read_payment_summary", …)`. Function `extract_payment_summary` matches between `payments.rs` and `commands.rs`. ✓
