# Generate a camt.053 statement from a payment file

Date: 2026-11-07

## Intent

The user sends pain.001 (credit transfer) and pain.008 (direct debit) files to
the bank and wants to see how the bank would report them back: the camt.053
end-of-day statements that would arrive once the bank had booked the file.
The generator turns a chosen payment file into such statements. They feed the
app's own camt views and checks (PR #25), and any other tool the user wants to
test against, with data that matches a file they actually built.

## Goals

- From a selected pain.001 or pain.008 file, produce **camt.053.001.08**
  statements as if the bank had booked the file.
- **One statement per booking day**, in date order. Days are chained: each
  day's closing balance (CLBD) is the next day's opening balance (OPBD).
- The opening balance of the first day is entered in a dialog, prefilled with
  **10,000.00 EUR**.
- Each generated file **validates against `camt.053.001.08.xsd`** and passes the
  app's statement checks in `app/src-tauri/src/statements.rs`:
  - OPBD plus booked entries equals CLBD.
  - `TxsSummry` counts and sums match the entries, including `TtlNetNtry`,
    `TtlCdtNtries` and `TtlDbtNtries`.
- Generated files are saved **next to the source file**. Afterwards they are
  added to the file list and validated straight away.

## Non-goals

- Statements with random entries that don't come from a payment file.
- Bank-side events: returns, rejects, R-transactions, fees, interest, FX.
- camt.052 or camt.054 output, and camt.053 versions other than .001.08.
- Batch generation for many source files at once.

## Anti-goals (a failure even if it technically works)

- Overwriting any existing file, above all the source file.
- Bank-added data that could pass for real (bank names, references, IDs).
- A statement that doesn't add up, or one the app itself flags.
- Any generated sample, or any real payment file, ending up in the repo
  (PII guard: no `.xml` outside `app/src-tauri/tests/fixtures/`).

## Decided with the user

- **Source:** a chosen pain.001 or pain.008 file. Random generation is not
  wanted.
- **Opening balance:** entered in a dialog, prefilled with 10,000.00 EUR.
- **Real data from the source stays.** Account IBAN, counterparty names and
  IBANs, EndToEndId and remittance text are copied from the source. The output
  stays local like the source does.
- **Sequence:** one statement per booking day, chained as described under
  Goals.
- **Location:** next to the source file.
- **Several accounts in one file** (answered after the reviewer check): one
  statement file per account and booking day. Each account has its own chain,
  and every chain starts from the same opening balance. The file name carries
  the last 4 characters of the IBAN (see File names).
- **File list after generating** (answered after the reviewer check): the
  statements are **appended** to the current list, only they are validated,
  and the first one is selected. The rest of the list stays as it is.

## Calls Claude made after the reviewer check

- **Source data that would break the output.** The dialog refuses the file and
  lists every reason. The data is never silently cleaned up. Refusal reasons:
  - not a parsable pain.001/pain.008;
  - no transactions;
  - a non-EUR amount;
  - a missing or malformed amount;
  - a PmtInf account without an IBAN;
  - an account or counterparty IBAN that fails mod-97, since the app would flag
    it in the statement;
  - a missing or malformed requested date.

  A counterparty without an IBAN (e.g. `Othr`) is booked without the account
  element. A missing EndToEndId is simply left out, since it's optional in
  camt.
- **Refusal goes by currency alone.** EUR files of every variant are booked:
  DK SEPA, AXZ, CCU and Swiss. `BkTxCd` is `PMNT/ICDT/ESCT` when the PmtInf
  service level is `SEPA` and `PMNT/ICDT/XBCT` otherwise. Direct debits get
  `PMNT/IDDT/ESDD`, or `PMNT/IDDT/BBDD` for local instrument `B2B`.
- Weekend dates are booked on the date as given; there's no calendar logic.
  "Today" is the UTC date the app already uses (`plausibility::today`). The
  opening balance may be negative, which is written as DBIT.

## Proposed by Claude and accepted in the playback

### Entry point

When the selected file is a pain.001 or pain.008, a **Generate statement…**
button appears in the viewer bar next to *Save formatted*. It opens a dialog
with three parts:

- the opening balance (editable, prefilled);
- a preview line, e.g. "3 booking days, 14 transactions";
- a *Generate* button.

If the file can't be booked, the dialog explains why and *Generate* stays
disabled. That happens when the file contains non-EUR transactions, or when it
has no transactions at all.

### Account

- **Account** = the debtor account of the PmtInf (`DbtrAcct`) for a pain.001,
  or the creditor account (`CdtrAcct`) for a pain.008.
- **Owner name:** the matching `Dbtr`/`Cdtr` `Nm`.
- **Servicer BIC:** the agent BIC (`DbtrAgt`/`CdtrAgt`) when the file has one.

### Direction

- pain.001 transactions are **DBIT** entries.
- pain.008 transactions are **CRDT** entries.

### Booking date

- Booking date = the PmtInf's `ReqdExctnDt` (pain.001; also its `Dt` child in
  .001.09) or `ReqdColltnDt` (pain.008).
- A date in the past becomes **today**. This includes the DK placeholder
  `1999-01-01`, which means "as soon as possible".
- The value date (`ValDt`) equals the booking date.

### Grouping

Grouping follows `BtchBookg` on the PmtInf:

- **true or missing:** one batch entry per PmtInf per booking day. Its amount
  is the sum of the PmtInf's transactions, and every transaction is listed as
  a `TxDtls` under it. `NtryDtls/Btch/NbOfTxs` gives the count.
- **false:** one entry per transaction, with a single `TxDtls`.

### Transaction details

Each `TxDtls` carries:

- `Refs/EndToEndId`, plus `InstrId` when present;
- the amount;
- the counterparty: `Cdtr`/`CdtrAcct` for a pain.001, `Dbtr`/`DbtrAcct` for a
  pain.008;
- `RmtInf/Ustrd`;
- the mandate ID and the creditor ID for a pain.008.

### Status and synthetic data

- Every entry has status **BOOK**.
- Everything the bank would add is visibly synthetic:
  - `GrpHdr/MsgId`, `Stmt/Id`, `AcctSvcrRef` and similar fields start with
    `SYNTH-`.
  - The servicer name, if one is written, says it is synthetic, e.g.
    "SYNTHETIC TEST BANK".
  - `AddtlNtryInf` or `AddtlStmtInf` states that the statement was generated
    from `<source file name>` by SEPA Validator.

### Currency

- Only EUR is booked. The account currency is EUR.
- A file with any non-EUR transaction, such as AXZ foreign payments, is
  refused with a clear message. Real banks would convert at an FX rate, and
  faking one would mislead.

### Source requirements

The source must be well-formed and parsable as pain.001 or pain.008. It does
not need to pass its XSD, because testing an imperfect file is a valid use.

### File names

- Pattern: `<source stem>_camt053_YYYYMMDD.xml` in the source's folder, where
  `YYYYMMDD` is the booking day. If the file covers several accounts, the
  pattern becomes `<source stem>_camt053_<last 4 of IBAN>_YYYYMMDD.xml`.
- On a clash, `_1`, `_2`, … is appended before `.xml`, following the repo's
  naming convention. Nothing is overwritten.
- Each target path goes through `check_export_path(…, &["xml"])`, the same
  guard `save_formatted` uses, and must never equal the source.

### Architecture

- **Backend:** a new Rust module, `app/src-tauri/src/statement_gen.rs` (name
  is the builder's call), which:
  - parses the payment file into booking days;
  - builds the camt.053 XML strings;
  - is pure and unit-testable, so file writing stays in `commands.rs`.
- **Commands** in `commands.rs`:
  - one that previews the source (booking days, transaction count, or the
    reason it can't be booked);
  - one that generates and writes the files, then returns the written paths.
  - The opening balance crosses IPC as a decimal string, not a float.
- **Frontend:**
  - a dialog component in the style of the existing dialogs
    (`SchemaDialog`, `AboutDialog`);
  - a button in the viewer bar in `App.svelte`.
  - After generating, the returned paths are added to the file list and
    validated with the existing validation flow.

### Tests

- Rust unit tests on synthetic in-memory pain.001 and pain.008 inputs (test
  IBANs with valid check digits, invented names). They cover:
  - day splitting and balance chaining;
  - both `BtchBookg` cases;
  - DBIT for pain.001 and CRDT for pain.008;
  - past dates and `1999-01-01` becoming today;
  - non-EUR refusal;
  - file-name collisions.
- Every generated statement is run through `statements::check`, which must
  return no warnings.
- XSD validation against `camt.053.001.08.xsd` is checked locally with the
  user's schema. The XSD isn't in the repo, so a CI test can't load it; it
  must skip when the XSD is absent, like the existing spike tests.

## Constraints

- Tauri v2, Rust backend, Svelte 5 frontend. Windows only.
- CI requires `cargo fmt --check`, `clippy -D warnings`, `cargo test`, and
  `npm run check` / `npm test`.
- No `.xml`/`.xsd` committed outside `app/src-tauri/tests/fixtures/`. Test
  inputs live inline in Rust tests or as synthetic fixtures there.
- Amounts are exact decimals. Use the existing `plausibility::Amount`, not
  floats.
- The camt namespace and version are fixed: camt.053.001.08.

## Left to the builder

- Module, command and component names; the exact layout of the XML.
- `BkTxCd` values (for example `PMNT`/`ICDT`/`ESCT` for SEPA credit transfers
  and `PMNT`/`IDDT`/`ESDD` for direct debits).
- How `ElctrncSeqNb`/`LglSeqNb` are numbered, e.g. 1…n per run.
- `CreDtTm` values, and the `FrToDt` of each day (that day, 00:00 to 23:59:59).
- Whether a booking day mixes entries from several PmtInf blocks (it should,
  if they share the date).
- The dialog's exact wording and layout. It should follow the existing dialogs.
