# Remittance Origin (InstrId/EndToEndId) + CSV Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In the Verwendungszweck tab, show each entry's origin (InstrId, falling back to EndToEndId) and make the table exportable as CSV.

**Architecture:** The backend `payments.rs` captures `PmtId/InstrId` and `PmtId/EndToEndId` per transaction into `RemittanceEntry`. The frontend turns the remittance list into a 3-column table (`# · Herkunft · Verwendungszweck`) and adds an Export-CSV button that writes the current file's rows via the existing save-dialog + `writeTextFile` pattern.

**Tech Stack:** Rust + Tauri v2, `quick-xml`; Svelte 5 + TypeScript, `@tauri-apps/plugin-dialog` `save`.

## Global Constraints

- `RemittanceEntry` gains `instr_id` and `end_to_end_id` (serde camelCase → `instrId`, `endToEndId`), both `Option<String>` → TS `string | null`; `ustrd` stays.
- Capture per transaction (`CdtTrfTxInf`/`DrctDbtTxInf`): `InstrId` = first `PmtId/InstrId`, `EndToEndId` = first `PmtId/EndToEndId` (only inside a transaction).
- Display "Herkunft" = `instrId ?? endToEndId ?? "—"`. The Verwendungszweck tab becomes a `<table>` (`# · Herkunft · Verwendungszweck`); missing `ustrd` → red "⚠ kein Verwendungszweck" cell; banner stays.
- Export: CSV only, current file only, `;`-separated with `"`-escaping (mirror existing `exportCsv`); header `#;Herkunft;Verwendungszweck`; missing `ustrd` → empty cell; multi-line `ustrd` newlines → single space.
- No JS test runner; frontend verification `npm run check` (0/0). Backend `cargo test`.
- Commit format: `type(scope): summary`.

---

### Task 1: Backend — capture InstrId / EndToEndId per transaction

**Files:**
- Modify: `app/src-tauri/src/payments.rs`

**Interfaces:**
- Produces: `RemittanceEntry { instr_id: Option<String>, end_to_end_id: Option<String>, ustrd: Option<String> }` (serde camelCase: `instrId`, `endToEndId`, `ustrd`).

- [ ] **Step 1: Extend the `RemittanceEntry` DTO**

In `app/src-tauri/src/payments.rs`, replace:

```rust
pub struct RemittanceEntry {
    pub ustrd: Option<String>,
}
```

with:

```rust
pub struct RemittanceEntry {
    pub instr_id: Option<String>,
    pub end_to_end_id: Option<String>,
    pub ustrd: Option<String>,
}
```

- [ ] **Step 2: Add a per-transaction accumulator type**

In `app/src-tauri/src/payments.rs`, directly above `pub fn extract_payment_summary`, add:

```rust
/// Per-transaction accumulation while parsing (not serialized).
#[derive(Default)]
struct TxAccum {
    ustrd: Vec<String>,
    instr_id: Option<String>,
    end_to_end_id: Option<String>,
}
```

- [ ] **Step 3: Switch `current_tx` to the accumulator**

Replace:

```rust
    let mut current_tx: Option<Vec<String>> = None;
```

with:

```rust
    let mut current_tx: Option<TxAccum> = None;
```

Replace the transaction-start arm:

```rust
                    "CdtTrfTxInf" | "DrctDbtTxInf" => current_tx = Some(Vec::new()),
```

with:

```rust
                    "CdtTrfTxInf" | "DrctDbtTxInf" => current_tx = Some(TxAccum::default()),
```

Replace the transaction-end arm:

```rust
                    "CdtTrfTxInf" | "DrctDbtTxInf" => {
                        if let Some(tx) = current_tx.take() {
                            let ustrd = if tx.is_empty() { None } else { Some(tx.join("\n")) };
                            transactions.push(RemittanceEntry { ustrd });
                        }
                    }
```

with:

```rust
                    "CdtTrfTxInf" | "DrctDbtTxInf" => {
                        if let Some(tx) = current_tx.take() {
                            let ustrd = if tx.ustrd.is_empty() {
                                None
                            } else {
                                Some(tx.ustrd.join("\n"))
                            };
                            transactions.push(RemittanceEntry {
                                instr_id: tx.instr_id,
                                end_to_end_id: tx.end_to_end_id,
                                ustrd,
                            });
                        }
                    }
```

- [ ] **Step 4: Capture InstrId/EndToEndId and push Ustrd into the accumulator**

In the `Event::Text` match, replace the `"Ustrd"` arm:

```rust
                    "Ustrd" => {
                        if let Some(tx) = current_tx.as_mut() {
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                tx.push(trimmed.to_string());
                            }
                        }
                    }
```

with (adds the two new arms before it; updates the Ustrd push to `tx.ustrd`):

```rust
                    "InstrId" if parent == "PmtId" => {
                        if let Some(tx) = current_tx.as_mut() {
                            tx.instr_id.get_or_insert(text);
                        }
                    }
                    "EndToEndId" if parent == "PmtId" => {
                        if let Some(tx) = current_tx.as_mut() {
                            tx.end_to_end_id.get_or_insert(text);
                        }
                    }
                    "Ustrd" => {
                        if let Some(tx) = current_tx.as_mut() {
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                tx.ustrd.push(trimmed.to_string());
                            }
                        }
                    }
```

- [ ] **Step 5: Add PmtId to the pain.001 test fixture and assert origin**

In the `PAIN001` test fixture, replace these two transaction lines:

```rust
      <CdtTrfTxInf><Cdtr><Nm>Payee One</Nm></Cdtr><RmtInf><Ustrd>Invoice 1</Ustrd></RmtInf></CdtTrfTxInf>
      <CdtTrfTxInf><Cdtr><Nm>Payee Two</Nm></Cdtr><RmtInf><Ustrd>Invoice 2</Ustrd></RmtInf></CdtTrfTxInf>
```

with (tx1 has InstrId+EndToEndId; tx2 has only EndToEndId — the fallback case):

```rust
      <CdtTrfTxInf><PmtId><InstrId>INSTR-1</InstrId><EndToEndId>E2E-1</EndToEndId></PmtId><Cdtr><Nm>Payee One</Nm></Cdtr><RmtInf><Ustrd>Invoice 1</Ustrd></RmtInf></CdtTrfTxInf>
      <CdtTrfTxInf><PmtId><EndToEndId>E2E-2</EndToEndId></PmtId><Cdtr><Nm>Payee Two</Nm></Cdtr><RmtInf><Ustrd>Invoice 2</Ustrd></RmtInf></CdtTrfTxInf>
```

(The third transaction — `Invoice 3` in the second PmtInf — keeps no `PmtId`, so both origin fields are `None`.)

Then in the test `pain001_blocks_transactions_and_no_pmtinf_creditor`, directly after the existing `assert_eq!(s.transactions[2].ustrd.as_deref(), Some("Invoice 3"));` line, add:

```rust
        // Origin: InstrId preferred; tx2 falls back to EndToEndId; tx3 has neither.
        assert_eq!(s.transactions[0].instr_id.as_deref(), Some("INSTR-1"));
        assert_eq!(s.transactions[0].end_to_end_id.as_deref(), Some("E2E-1"));
        assert_eq!(s.transactions[1].instr_id, None);
        assert_eq!(s.transactions[1].end_to_end_id.as_deref(), Some("E2E-2"));
        assert_eq!(s.transactions[2].instr_id, None);
        assert_eq!(s.transactions[2].end_to_end_id, None);
```

- [ ] **Step 6: Build + test**

Run: `cd app/src-tauri && cargo test`
Expected: all tests pass (the extended `pain001…` test included), no warnings.

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/src/payments.rs
git commit -m "feat(app): capture InstrId/EndToEndId per transaction"
```

---

### Task 2: Frontend — origin column + CSV export

**Files:**
- Modify: `app/src/lib/types.ts`
- Modify: `app/src/lib/export.ts`
- Modify: `app/src/lib/RemittanceView.svelte`

**Interfaces:**
- Consumes: the `instrId`/`endToEndId` JSON from Task 1; existing `paymentSummary` store, `selectedResult`, `save`/`writeTextFile`.
- Produces: `exportRemittanceCsv(transactions: RemittanceEntry[], sourceFile: string): Promise<void>`.

- [ ] **Step 1: Extend the TS type**

In `app/src/lib/types.ts`, replace:

```ts
export interface RemittanceEntry {
  ustrd: string | null;
}
```

with:

```ts
export interface RemittanceEntry {
  instrId: string | null;
  endToEndId: string | null;
  ustrd: string | null;
}
```

- [ ] **Step 2: Add the CSV export function**

In `app/src/lib/export.ts`, change the type import:

```ts
import type { ValidationResult } from "./types";
```

to:

```ts
import type { ValidationResult, RemittanceEntry } from "./types";
```

Then append at the end of the file:

```ts
export async function exportRemittanceCsv(
  transactions: RemittanceEntry[],
  sourceFile: string
): Promise<void> {
  const base = sourceFile.replace(/\.[^.]+$/, "") || "datei";
  const path = await save({
    defaultPath: `Verwendungszweck_${base}_${stamp()}.csv`,
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  if (!path) return;
  const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;
  let out = "#;Herkunft;Verwendungszweck\n";
  transactions.forEach((t, i) => {
    const herkunft = t.instrId ?? t.endToEndId ?? "";
    const zweck = (t.ustrd ?? "").replace(/\r?\n/g, " ");
    out += [i + 1, esc(herkunft), esc(zweck)].join(";") + "\n";
  });
  await writeTextFile(path, out);
}
```

- [ ] **Step 3: Turn RemittanceView into a table with an export button**

In `app/src/lib/RemittanceView.svelte`, change the `<script>` to import the export helper and add an export handler. Replace:

```svelte
<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  $: ps = $paymentSummary;
</script>
```

with:

```svelte
<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  import { exportRemittanceCsv } from "./export";
  $: ps = $paymentSummary;

  async function doExport() {
    const tx = ps.data?.transactions ?? [];
    if (tx.length) await exportRemittanceCsv(tx, $selectedResult?.file ?? "datei");
  }
</script>
```

Then replace the `{:else}` body that renders the list:

```svelte
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
```

with:

```svelte
    {:else}
      {#if missing > 0}
        <p class="warn-banner">⚠ {missing} von {total} Transaktionen ohne Verwendungszweck</p>
      {/if}
      <div class="rmt-toolbar">
        <button on:click={doExport}>Export CSV</button>
      </div>
      <table>
        <thead><tr><th>#</th><th>Herkunft</th><th>Verwendungszweck</th></tr></thead>
        <tbody>
          {#each data.transactions as t, i}
            <tr>
              <td>{i + 1}</td>
              <td>{t.instrId ?? t.endToEndId ?? "—"}</td>
              <td class={t.ustrd == null ? "missing" : ""}>{t.ustrd ?? "⚠ kein Verwendungszweck"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
```

- [ ] **Step 4: Update the styles**

In `app/src/lib/RemittanceView.svelte`, replace the three `ol.ustrd` style rules:

```css
  ol.ustrd { margin: 0; padding-left: 22px; font-size: 13px; }
  ol.ustrd li { padding: 2px 0; word-break: break-word; white-space: pre-wrap; }
  ol.ustrd li.missing { color: var(--err); font-style: italic; }
```

with:

```css
  .rmt-toolbar { margin: 0 0 8px; }
  .rmt-toolbar button { background: var(--accent); color: #fff; border: none; padding: 4px 10px; border-radius: 6px; cursor: pointer; font-size: 12px; }
  .rmt-toolbar button:hover { filter: brightness(1.1); }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--border); vertical-align: top; }
  th { font-weight: 600; }
  td { word-break: break-word; white-space: pre-wrap; }
  td.missing { color: var(--err); font-style: italic; }
```

- [ ] **Step 5: Type-check**

Run: `cd app && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/types.ts app/src/lib/export.ts app/src/lib/RemittanceView.svelte
git commit -m "feat(app): remittance origin column + CSV export"
```

---

## Self-Review

**Spec coverage:**
- Origin captured per transaction (InstrId + EndToEndId, first occurrence, inside a tx) → Task 1 Steps 2-4.
- Fallback display `instrId ?? endToEndId ?? "—"` → Task 2 Step 3 + export Step 2.
- Verwendungszweck tab becomes a 3-column table; missing ustrd → red cell; banner stays → Task 2 Step 3.
- CSV only, current file, `;`-separated, `"`-escaped, header `#;Herkunft;Verwendungszweck`, missing ustrd → empty cell, multi-line → space → Task 2 Step 2.
- DTO camelCase ↔ TS → Task 1 (serde) + Task 2 Step 1.
- Tests: InstrId+EndToEndId, fallback (EndToEndId only), neither → Task 1 Step 5.
- YAGNI exclusions (TXT, cross-file, overview export, split columns) → not implemented. ✓

**Placeholder scan:** No TBD/TODO; every step shows exact code. ✓

**Type consistency:** Rust `instr_id`/`end_to_end_id`/`ustrd` serialize to `instrId`/`endToEndId`/`ustrd`, matching the TS `RemittanceEntry` (Task 2 Step 1) and the usages `t.instrId`/`t.endToEndId`/`t.ustrd` in RemittanceView + `exportRemittanceCsv`. `TxAccum` is internal-only (not serialized). `exportRemittanceCsv(transactions, sourceFile)` signature matches the call `exportRemittanceCsv(tx, $selectedResult?.file ?? "datei")`. ✓
