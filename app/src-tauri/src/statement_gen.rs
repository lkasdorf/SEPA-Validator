//! Synthetic camt.053.001.08 bank statements from a pain.001/pain.008 file,
//! as if the bank had booked it: one statement per account and booking day,
//! days chained (each closing balance opens the next day). Everything the bank
//! would add (IDs, references, the bank itself) is marked synthetic; data from
//! the source file (IBANs, names, references, remittance text) is copied.

use serde::Serialize;

use crate::plausibility::{iban_is_valid, Amount};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    CreditTransfer,
    DirectDebit,
}

/// One transaction. The counterparty is `Cdtr`/`CdtrAcct` for a credit
/// transfer and `Dbtr`/`DbtrAcct` for a direct debit.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Tx {
    pub instr_id: Option<String>,
    pub end_to_end_id: Option<String>,
    pub amount: Amount,
    pub name: Option<String>,
    pub iban: Option<String>,
    pub ustrd: Vec<String>,
    pub mandate_id: Option<String>,
    /// Service level SEPA / local instrument B2B: the transaction's own
    /// `PmtTpInf` if it has one, else the PmtInf's.
    pub sepa: bool,
    pub b2b: bool,
}

/// One PmtInf block. `iban`/`owner`/`bic` describe the booked account
/// (`DbtrAcct` for pain.001, `CdtrAcct` for pain.008); `date` is the
/// resolved booking date.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Block {
    pub pmt_inf_id: Option<String>,
    pub iban: String,
    pub owner: Option<String>,
    pub bic: Option<String>,
    pub creditor_id: Option<String>,
    pub batch: bool,
    pub sepa: bool,
    pub b2b: bool,
    pub date: String,
    pub txs: Vec<Tx>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Payment {
    pub kind: Kind,
    pub msg_id: Option<String>,
    pub blocks: Vec<Block>,
}

/// A transaction while it is being read.
#[derive(Default)]
struct RawTx {
    tx: Tx,
    amount: Option<String>,
    currency: Option<String>,
    equivalent: bool,
    creditor_id: Option<String>,
    sepa: Option<bool>,
    b2b: Option<bool>,
}

/// A PmtInf block while it is being read.
#[derive(Default)]
struct RawBlock {
    block: Block,
    iban: Option<String>,
    date: Option<String>,
    batch_off: bool,
}

fn label(id: Option<&str>, n: usize) -> String {
    id.map_or_else(|| format!("#{n}"), |s| format!("\"{s}\""))
}

/// The booking day for a requested date: past dates and the DK placeholder
/// `1999-01-01` ("as soon as possible") are booked today.
fn resolve_date(raw: &str, today: &str) -> Option<String> {
    let d = raw.get(..10)?;
    let b = d.as_bytes();
    let shape = b.iter().enumerate().all(|(i, c)| {
        if i == 4 || i == 7 {
            *c == b'-'
        } else {
            c.is_ascii_digit()
        }
    });
    if !shape {
        return None;
    }
    Some(if d == "1999-01-01" || d < today {
        today.to_string()
    } else {
        d.to_string()
    })
}

/// Parse a pain.001/pain.008 file. `today` is `YYYY-MM-DD`. `Err` lists every
/// reason the file can't be booked, each a complete sentence.
pub(crate) fn parse_payment(xml: &str, today: &str) -> Result<Payment, Vec<String>> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    const NOT_PAYMENT: &str = "This is not a pain.001 or pain.008 payment file.";
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut kind: Option<Kind> = None;
    let mut msg_id = None;
    let mut blocks: Vec<Block> = Vec::new();
    let mut reasons: Vec<String> = Vec::new();
    let mut block: Option<RawBlock> = None;
    let mut tx: Option<RawTx> = None;
    let (mut n_blocks, mut n_txs) = (0usize, 0usize);

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                match stack.len() {
                    1 => {
                        kind = match name.as_str() {
                            "CstmrCdtTrfInitn" => Some(Kind::CreditTransfer),
                            "CstmrDrctDbtInitn" => Some(Kind::DirectDebit),
                            _ => return Err(vec![NOT_PAYMENT.into()]),
                        }
                    }
                    2 if name == "PmtInf" => {
                        n_blocks += 1;
                        block = Some(RawBlock::default());
                    }
                    3 if name == "CdtTrfTxInf" || name == "DrctDbtTxInf" => {
                        n_txs += 1;
                        tx = Some(RawTx::default());
                    }
                    _ => {}
                }
                if let Some(t) = tx.as_mut() {
                    let p: Vec<&str> = stack.iter().skip(4).map(String::as_str).collect();
                    match (p.as_slice(), name.as_str()) {
                        (["Amt"] | [], "InstdAmt") => {
                            t.currency = e
                                .attributes()
                                .flatten()
                                .find(|a| a.key.local_name().as_ref() == b"Ccy")
                                .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()));
                        }
                        (["Amt"], "EqvtAmt") => t.equivalent = true,
                        _ => {}
                    }
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                if stack.len() == 1 {
                    let name = e.local_name();
                    if name.as_ref() != b"CstmrCdtTrfInitn" && name.as_ref() != b"CstmrDrctDbtInitn"
                    {
                        return Err(vec![NOT_PAYMENT.into()]);
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let Ok(v) = t.unescape() else { continue };
                let v = v.trim();
                if v.is_empty() {
                    continue;
                }
                let p: Vec<&str> = stack.iter().map(String::as_str).collect();
                if let ([_, _, "GrpHdr", "MsgId"], None) = (p.as_slice(), &block) {
                    msg_id = Some(v.to_string());
                }
                if let Some(t) = tx.as_mut() {
                    tx_text(t, &p[4..], v);
                } else if let Some(b) = block.as_mut() {
                    block_text(b, &p[3..], v);
                }
            }
            Ok(Event::End(_)) => {
                let name = stack.pop().unwrap_or_default();
                if stack.len() == 3 && (name == "CdtTrfTxInf" || name == "DrctDbtTxInf") {
                    if let (Some(t), Some(b)) = (tx.take(), block.as_mut()) {
                        finish_tx(t, n_txs, b, &mut reasons);
                    }
                } else if stack.len() == 2 && name == "PmtInf" {
                    if let Some(b) = block.take() {
                        finish_block(b, n_blocks, today, &mut blocks, &mut reasons);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(vec![format!("The file could not be read as XML: {e}.")]),
            Ok(_) => {}
        }
    }
    let Some(kind) = kind else {
        return Err(vec![NOT_PAYMENT.into()]);
    };
    if n_txs == 0 {
        reasons.push("The file contains no transactions.".into());
    }
    if reasons.is_empty() {
        Ok(Payment {
            kind,
            msg_id,
            blocks,
        })
    } else {
        Err(reasons)
    }
}

/// Text inside a PmtInf (outside its transactions); `p` is the path below PmtInf.
fn block_text(b: &mut RawBlock, p: &[&str], v: &str) {
    let set = |o: &mut Option<String>| {
        o.get_or_insert_with(|| v.to_string());
    };
    match p {
        ["PmtInfId"] => set(&mut b.block.pmt_inf_id),
        ["BtchBookg"] => b.batch_off = v == "false" || v == "0",
        ["PmtTpInf", "SvcLvl", "Cd"] => b.block.sepa = v == "SEPA",
        ["PmtTpInf", "LclInstrm", "Cd"] => b.block.b2b = v == "B2B",
        ["ReqdExctnDt" | "ReqdColltnDt"] | ["ReqdExctnDt" | "ReqdColltnDt", "Dt" | "DtTm"] => {
            set(&mut b.date)
        }
        ["Dbtr" | "Cdtr", "Nm"] => set(&mut b.block.owner),
        ["DbtrAcct" | "CdtrAcct", "Id", "IBAN"] => set(&mut b.iban),
        ["DbtrAgt" | "CdtrAgt", "FinInstnId", "BIC" | "BICFI"] => set(&mut b.block.bic),
        ["CdtrSchmeId", "Id", "PrvtId", "Othr", "Id"] => set(&mut b.block.creditor_id),
        _ => {}
    }
}

/// Text inside a transaction; `p` is the path below CdtTrfTxInf/DrctDbtTxInf.
/// The counterparty is whichever of Cdtr/Dbtr appears at transaction level.
fn tx_text(t: &mut RawTx, p: &[&str], v: &str) {
    let set = |o: &mut Option<String>| {
        o.get_or_insert_with(|| v.to_string());
    };
    match p {
        ["PmtId", "InstrId"] => set(&mut t.tx.instr_id),
        ["PmtId", "EndToEndId"] => set(&mut t.tx.end_to_end_id),
        ["Amt", "InstdAmt"] | ["InstdAmt"] => set(&mut t.amount),
        ["Cdtr" | "Dbtr", "Nm"] => set(&mut t.tx.name),
        ["CdtrAcct" | "DbtrAcct", "Id", "IBAN"] => set(&mut t.tx.iban),
        ["DrctDbtTx", "MndtRltdInf", "MndtId"] => set(&mut t.tx.mandate_id),
        ["DrctDbtTx", "CdtrSchmeId", "Id", "PrvtId", "Othr", "Id"] => set(&mut t.creditor_id),
        ["RmtInf", "Ustrd"] => t.tx.ustrd.push(v.to_string()),
        ["PmtTpInf", "SvcLvl", "Cd"] => t.sepa = Some(v == "SEPA"),
        ["PmtTpInf", "LclInstrm", "Cd"] => t.b2b = Some(v == "B2B"),
        _ => {}
    }
}

fn finish_tx(t: RawTx, n: usize, b: &mut RawBlock, reasons: &mut Vec<String>) {
    let RawTx {
        mut tx,
        amount,
        currency,
        equivalent,
        creditor_id,
        sepa,
        b2b,
    } = t;
    // The PmtInf's PmtTpInf precedes its transactions, so it is known here.
    tx.sepa = sepa.unwrap_or(b.block.sepa);
    tx.b2b = b2b.unwrap_or(b.block.b2b);
    let what = format!("Transaction {}", label(tx.end_to_end_id.as_deref(), n));
    let before = reasons.len();
    if equivalent {
        reasons.push(format!(
            "{what} uses an equivalent amount (EqvtAmt) with a currency conversion; only EUR amounts can be booked."
        ));
    } else {
        match currency.as_deref() {
            Some("EUR") => {}
            Some(c) => reasons.push(format!("{what} is in {c}; only EUR can be booked.")),
            None => reasons.push(format!("{what} has no currency; only EUR can be booked.")),
        }
        match amount.as_deref().map(|a| (a, Amount::parse(a))) {
            Some((_, Some(a))) => tx.amount = a,
            Some((raw, None)) => {
                reasons.push(format!("{what} has an unreadable amount \"{raw}\"."))
            }
            None if currency.is_some() => reasons.push(format!("{what} has no amount.")),
            None => {}
        }
    }
    if let Some(iban) = tx.iban.as_deref().filter(|i| !iban_is_valid(i)) {
        reasons.push(format!(
            "{what}: the IBAN {iban} fails the check digit test."
        ));
    }
    if b.block.creditor_id.is_none() {
        b.block.creditor_id = creditor_id;
    }
    if reasons.len() == before {
        b.block.txs.push(tx);
    }
}

fn finish_block(
    b: RawBlock,
    n: usize,
    today: &str,
    blocks: &mut Vec<Block>,
    reasons: &mut Vec<String>,
) {
    let RawBlock {
        mut block,
        iban,
        date,
        batch_off,
    } = b;
    let what = format!("Payment block {}", label(block.pmt_inf_id.as_deref(), n));
    match iban {
        Some(i) if iban_is_valid(&i) => block.iban = i,
        Some(i) => reasons.push(format!(
            "{what}: the account IBAN {i} fails the check digit test."
        )),
        None => reasons.push(format!(
            "{what} has no account IBAN; a statement needs one."
        )),
    }
    match date.as_deref().and_then(|d| resolve_date(d, today)) {
        Some(d) => block.date = d,
        None => reasons.push(format!(
            "{what} has no valid requested date ({}).",
            date.as_deref().unwrap_or("missing")
        )),
    }
    block.batch = !batch_off;
    blocks.push(block);
}

/// The opening balance as typed: `10000.00`, `10,000.00`, `-250.5`.
/// Commas are only accepted as thousands separators (groups of 3 before the
/// point), so a German `12,50` or `10.000,00` is refused, not misread.
/// Signed: a negative amount is a debit balance.
pub(crate) fn parse_opening(s: &str) -> Result<Amount, String> {
    let err = || {
        format!(
            "\"{}\" is not an amount. Enter the opening balance like 10000.00.",
            s.trim()
        )
    };
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let (negative, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.as_str()),
    };
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    let groups: Vec<&str> = int.split(',').collect();
    let grouped_ok = groups.len() == 1
        || (!groups[0].is_empty()
            && groups[0].len() <= 3
            && groups[1..].iter().all(|g| g.len() == 3));
    if !grouped_ok || frac.len() > 2 || (body.contains('.') && frac.is_empty()) {
        return Err(err());
    }
    let plain = if frac.is_empty() {
        groups.concat()
    } else {
        format!("{}.{frac}", groups.concat())
    };
    let a = Amount::parse(&plain).ok_or_else(err)?;
    Ok(if negative { Amount(-a.0) } else { a })
}

/// One generated statement: the booked account, the booking day and the XML.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Generated {
    pub iban: String,
    /// Short account tag for file names and synthetic IDs: the IBAN's last 4
    /// characters, more if two accounts of the file would share them.
    pub tag: String,
    pub date: String,
    pub xml: String,
}

/// One tag per IBAN (same order): the shortest common suffix length from 4 to
/// 8 that tells all accounts apart, else the last 4 plus `-<n>`. At most 8
/// characters for up to 99 accounts, so the synthetic IDs stay within Max35Text.
fn account_tags(ibans: &[&str]) -> Vec<String> {
    let suffix = |iban: &str, n: usize| iban[iban.len().saturating_sub(n)..].to_string();
    for n in 4..=8 {
        let tags: Vec<String> = ibans.iter().map(|i| suffix(i, n)).collect();
        let distinct: std::collections::BTreeSet<&String> = tags.iter().collect();
        if distinct.len() == tags.len() {
            return tags;
        }
    }
    ibans
        .iter()
        .enumerate()
        .map(|(i, iban)| format!("{}-{}", suffix(iban, 4), i + 1))
        .collect()
}

/// What `generate` would produce, for the dialog. With `problems` the file
/// can't be booked and every count is 0.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementPreview {
    pub kind: String,
    pub accounts: u32,
    pub days: u32,
    pub statements: u32,
    pub transactions: u32,
    pub problems: Vec<String>,
}

/// The statements for `xml`, ordered by account (first appearance) and date.
/// `opening` is the first day's opening balance (signed); `now` is
/// `YYYY-MM-DDTHH:MM:SS`, used for `CreDtTm` and the message IDs.
pub(crate) fn generate(
    xml: &str,
    source_name: &str,
    opening: Amount,
    today: &str,
    now: &str,
) -> Result<Vec<Generated>, Vec<String>> {
    let p = parse_payment(xml, today)?;
    let note: String = format!("Synthetic booking generated by SEPA Validator from {source_name}")
        .chars()
        .take(500)
        .collect();
    let mut out = Vec::new();
    let groups = group(&p);
    let tags = account_tags(&groups.iter().map(|(iban, _)| *iban).collect::<Vec<_>>());
    for ((iban, days), tag) in groups.iter().zip(&tags) {
        let mut balance = opening;
        for (seq, (date, blocks)) in days.iter().enumerate() {
            let ctx = Day {
                payment: &p,
                iban,
                tag,
                date,
                blocks,
                opening: balance,
                seq: seq + 1,
                now,
                note: &note,
            };
            let (xml, closing) = render(&ctx);
            out.push(Generated {
                iban: iban.to_string(),
                tag: tag.clone(),
                date: date.to_string(),
                xml,
            });
            balance = closing;
        }
    }
    Ok(out)
}

pub(crate) fn preview(xml: &str, today: &str) -> StatementPreview {
    let empty = |problems| StatementPreview {
        kind: String::new(),
        accounts: 0,
        days: 0,
        statements: 0,
        transactions: 0,
        problems,
    };
    let p = match parse_payment(xml, today) {
        Ok(p) => p,
        Err(problems) => return empty(problems),
    };
    let groups = group(&p);
    let dates: std::collections::BTreeSet<&str> = groups
        .iter()
        .flat_map(|(_, days)| days.iter().map(|(d, _)| *d))
        .collect();
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    StatementPreview {
        kind: match p.kind {
            Kind::CreditTransfer => "pain.001",
            Kind::DirectDebit => "pain.008",
        }
        .into(),
        accounts: count(groups.len()),
        days: count(dates.len()),
        statements: count(groups.iter().map(|(_, d)| d.len()).sum()),
        transactions: count(p.blocks.iter().map(|b| b.txs.len()).sum()),
        problems: Vec::new(),
    }
}

/// The current UTC time as `YYYY-MM-DDTHH:MM:SS`.
pub(crate) fn now_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = crate::plausibility::civil_from_days((secs / 86_400) as i64);
    let t = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        t / 3600,
        t / 60 % 60,
        t % 60
    )
}

/// Blocks per account (first appearance) and booking day (ascending).
/// Blocks without transactions book nothing and are left out.
type Days<'a> = Vec<(&'a str, Vec<&'a Block>)>;

fn group(p: &Payment) -> Vec<(&str, Days<'_>)> {
    let mut accounts: Vec<(&str, std::collections::BTreeMap<&str, Vec<&Block>>)> = Vec::new();
    for b in p.blocks.iter().filter(|b| !b.txs.is_empty()) {
        let i = match accounts.iter().position(|(iban, _)| *iban == b.iban) {
            Some(i) => i,
            None => {
                accounts.push((&b.iban, Default::default()));
                accounts.len() - 1
            }
        };
        accounts[i].1.entry(&b.date).or_default().push(b);
    }
    accounts
        .into_iter()
        .map(|(iban, days)| (iban, days.into_iter().collect()))
        .collect()
}

/// Everything one statement is rendered from.
struct Day<'a> {
    payment: &'a Payment,
    iban: &'a str,
    tag: &'a str,
    date: &'a str,
    blocks: &'a [&'a Block],
    opening: Amount,
    seq: usize,
    now: &'a str,
    note: &'a str,
}

/// One booked entry: a whole batch, or a single transaction.
struct Entry<'a> {
    block: &'a Block,
    txs: Vec<&'a Tx>,
    batch: bool,
}

impl Entry<'_> {
    fn amount(&self) -> Amount {
        Amount(self.txs.iter().map(|t| t.amount.0).sum())
    }
}

/// Indented XML text, escaping every value.
struct W(String, usize);

impl W {
    fn indent(&mut self) {
        self.0.push_str(&"  ".repeat(self.1));
    }
    fn open(&mut self, tag: &str) {
        self.indent();
        self.0 += &format!("<{tag}>\n");
        self.1 += 1;
    }
    fn close(&mut self, tag: &str) {
        self.1 -= 1;
        self.indent();
        self.0 += &format!("</{tag}>\n");
    }
    fn leaf(&mut self, tag: &str, value: &str) {
        self.indent();
        self.0 += &format!("<{tag}>{}</{tag}>\n", quick_xml::escape::escape(value));
    }
    fn opt(&mut self, tag: &str, value: Option<&str>) {
        if let Some(v) = value {
            self.leaf(tag, v);
        }
    }
    fn amt(&mut self, tag: &str, a: Amount) {
        self.indent();
        self.0 += &format!("<{tag} Ccy=\"EUR\">{}</{tag}>\n", a.display());
    }
    /// `Amt` + `CdtDbtInd` of a signed balance.
    fn signed(&mut self, a: Amount) {
        self.amt("Amt", Amount(a.0.abs()));
        self.leaf("CdtDbtInd", if a.0 < 0 { "DBIT" } else { "CRDT" });
    }
    fn iban_account(&mut self, tag: &str, iban: &str) {
        self.open(tag);
        self.open("Id");
        self.leaf("IBAN", iban);
        self.close("Id");
        self.close(tag);
    }
}

fn balance(w: &mut W, code: &str, a: Amount, date: &str) {
    w.open("Bal");
    w.open("Tp");
    w.open("CdOrPrtry");
    w.leaf("Cd", code);
    w.close("CdOrPrtry");
    w.close("Tp");
    w.signed(a);
    w.open("Dt");
    w.leaf("Dt", date);
    w.close("Dt");
    w.close("Bal");
}

/// The statement XML and its closing balance.
fn render(d: &Day) -> (String, Amount) {
    let debit = d.payment.kind == Kind::CreditTransfer;
    let ind = if debit { "DBIT" } else { "CRDT" };
    let entries: Vec<Entry> = d
        .blocks
        .iter()
        .flat_map(|b| {
            if b.batch {
                vec![Entry {
                    block: b,
                    txs: b.txs.iter().collect(),
                    batch: true,
                }]
            } else {
                b.txs
                    .iter()
                    .map(|t| Entry {
                        block: b,
                        txs: vec![t],
                        batch: false,
                    })
                    .collect()
            }
        })
        .collect();
    let total = Amount(entries.iter().map(|e| e.amount().0).sum());
    let closing = Amount(d.opening.0 + if debit { -total.0 } else { total.0 });
    let tag = d.tag;
    let day = d.date.replace('-', "");
    let owner = d.blocks.iter().find_map(|b| b.owner.as_deref());
    let bic = d.blocks.iter().find_map(|b| b.bic.as_deref());
    let n = entries.len().to_string();
    let zero = Amount(0);

    let mut w = W(
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
        0,
    );
    w.0 += "<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:camt.053.001.08\">\n";
    w.1 = 1;
    w.open("BkToCstmrStmt");
    w.open("GrpHdr");
    let stamp = d.now.replace(['-', ':'], "");
    w.leaf("MsgId", &format!("SYNTH-{stamp}-{tag}-{:03}", d.seq));
    w.leaf("CreDtTm", d.now);
    w.close("GrpHdr");
    w.open("Stmt");
    w.leaf("Id", &format!("SYNTH-{day}-{tag}-{:03}", d.seq));
    w.leaf("ElctrncSeqNb", &d.seq.to_string());
    w.leaf("CreDtTm", d.now);
    w.open("FrToDt");
    w.leaf("FrDtTm", &format!("{}T00:00:00", d.date));
    w.leaf("ToDtTm", &format!("{}T23:59:59", d.date));
    w.close("FrToDt");
    w.open("Acct");
    w.open("Id");
    w.leaf("IBAN", d.iban);
    w.close("Id");
    w.leaf("Ccy", "EUR");
    if let Some(o) = owner {
        w.open("Ownr");
        w.leaf("Nm", o);
        w.close("Ownr");
    }
    w.open("Svcr");
    w.open("FinInstnId");
    w.opt("BICFI", bic);
    w.leaf("Nm", "SYNTHETIC TEST BANK");
    w.close("FinInstnId");
    w.close("Svcr");
    w.close("Acct");
    balance(&mut w, "OPBD", d.opening, d.date);
    balance(&mut w, "CLBD", closing, d.date);

    w.open("TxsSummry");
    w.open("TtlNtries");
    w.leaf("NbOfNtries", &n);
    w.leaf("Sum", &total.display());
    w.open("TtlNetNtry");
    w.leaf("Amt", &total.display());
    w.leaf("CdtDbtInd", ind);
    w.close("TtlNetNtry");
    w.close("TtlNtries");
    for (tag, side) in [("TtlCdtNtries", false), ("TtlDbtNtries", true)] {
        let mine = side == debit;
        w.open(tag);
        w.leaf("NbOfNtries", if mine { &n } else { "0" });
        w.leaf("Sum", &(if mine { total } else { zero }).display());
        w.close(tag);
    }
    w.close("TxsSummry");

    for (i, e) in entries.iter().enumerate() {
        entry(
            &mut w,
            d,
            e,
            ind,
            owner,
            &format!("SYNTH-{day}-{tag}-E{}", i + 1),
        );
    }
    w.leaf("AddtlStmtInf", d.note);
    w.close("Stmt");
    w.close("BkToCstmrStmt");
    w.0 += "</Document>\n";
    (w.0, closing)
}

fn entry(w: &mut W, d: &Day, e: &Entry, ind: &str, owner: Option<&str>, reference: &str) {
    let debit = d.payment.kind == Kind::CreditTransfer;
    let b = e.block;
    w.open("Ntry");
    w.amt("Amt", e.amount());
    w.leaf("CdtDbtInd", ind);
    w.open("Sts");
    w.leaf("Cd", "BOOK");
    w.close("Sts");
    for tag in ["BookgDt", "ValDt"] {
        w.open(tag);
        w.leaf("Dt", d.date);
        w.close(tag);
    }
    w.leaf("AcctSvcrRef", reference);
    let first = e.txs[0];
    let (family, sub) = match (debit, first.sepa, first.b2b) {
        (true, true, _) => ("ICDT", "ESCT"),
        (true, false, _) => ("ICDT", "XBCT"),
        (false, _, true) => ("IDDT", "BBDD"),
        (false, _, false) => ("IDDT", "ESDD"),
    };
    w.open("BkTxCd");
    w.open("Domn");
    w.leaf("Cd", "PMNT");
    w.open("Fmly");
    w.leaf("Cd", family);
    w.leaf("SubFmlyCd", sub);
    w.close("Fmly");
    w.close("Domn");
    w.close("BkTxCd");
    w.open("NtryDtls");
    if e.batch {
        w.open("Btch");
        w.opt("PmtInfId", b.pmt_inf_id.as_deref());
        w.leaf("NbOfTxs", &e.txs.len().to_string());
        w.amt("TtlAmt", e.amount());
        w.leaf("CdtDbtInd", ind);
        w.close("Btch");
    }
    for t in &e.txs {
        w.open("TxDtls");
        w.open("Refs");
        w.opt("MsgId", d.payment.msg_id.as_deref());
        w.opt("PmtInfId", b.pmt_inf_id.as_deref());
        w.opt("InstrId", t.instr_id.as_deref());
        w.opt("EndToEndId", t.end_to_end_id.as_deref());
        w.opt("MndtId", t.mandate_id.as_deref());
        w.close("Refs");
        w.amt("Amt", t.amount);
        w.leaf("CdtDbtInd", ind);
        w.open("RltdPties");
        // Own account on the paying side for transfers, the receiving side for debits.
        let (dbtr, dbtr_iban, cdtr, cdtr_iban) = if debit {
            (owner, Some(d.iban), t.name.as_deref(), t.iban.as_deref())
        } else {
            (t.name.as_deref(), t.iban.as_deref(), owner, Some(d.iban))
        };
        let creditor_id = if debit {
            None
        } else {
            b.creditor_id.as_deref()
        };
        if let Some(nm) = dbtr {
            w.open("Dbtr");
            w.open("Pty");
            w.leaf("Nm", nm);
            w.close("Pty");
            w.close("Dbtr");
        }
        if let Some(i) = dbtr_iban {
            w.iban_account("DbtrAcct", i);
        }
        if cdtr.is_some() || creditor_id.is_some() {
            w.open("Cdtr");
            w.open("Pty");
            w.opt("Nm", cdtr);
            if let Some(id) = creditor_id {
                w.open("Id");
                w.open("PrvtId");
                w.open("Othr");
                w.leaf("Id", id);
                w.open("SchmeNm");
                w.leaf("Prtry", "SEPA");
                w.close("SchmeNm");
                w.close("Othr");
                w.close("PrvtId");
                w.close("Id");
            }
            w.close("Pty");
            w.close("Cdtr");
        }
        if let Some(i) = cdtr_iban {
            w.iban_account("CdtrAcct", i);
        }
        w.close("RltdPties");
        if !t.ustrd.is_empty() {
            w.open("RmtInf");
            for u in &t.ustrd {
                w.leaf("Ustrd", u);
            }
            w.close("RmtInf");
        }
        w.close("TxDtls");
    }
    w.close("NtryDtls");
    w.leaf("AddtlNtryInf", d.note);
    w.close("Ntry");
}

#[cfg(test)]
mod gen_tests {
    use super::tests::*;
    use super::*;

    const NOW: &str = "2026-11-07T09:30:00";
    const SRC: &str = "20261107_TEST_PAIN001.xml";

    fn eur(s: &str) -> Amount {
        Amount::parse(s).unwrap()
    }

    /// Texts of all elements whose path ends with `suffix`, in document order.
    fn texts(xml: &str, suffix: &[&str]) -> Vec<String> {
        use quick_xml::events::Event;
        let mut r = quick_xml::reader::Reader::from_str(xml);
        let mut stack: Vec<String> = Vec::new();
        let mut out = Vec::new();
        loop {
            match r.read_event().expect("generated XML is well-formed") {
                Event::Start(e) => {
                    stack.push(String::from_utf8_lossy(e.local_name().as_ref()).into_owned())
                }
                Event::End(_) => {
                    stack.pop();
                }
                Event::Text(t) => {
                    let v = t.unescape().unwrap();
                    if !v.trim().is_empty()
                        && stack
                            .ends_with(&suffix.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                    {
                        out.push(v.trim().to_string());
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        out
    }

    fn assert_clean(gens: &[Generated]) {
        assert!(!gens.is_empty());
        for g in gens {
            assert_eq!(crate::statements::check(&g.xml), vec![], "{}", g.xml);
            assert_eq!(
                crate::plausibility::check(&g.xml, TODAY),
                vec![],
                "{}",
                g.xml
            );
        }
    }

    fn gen(xml: &str, opening: &str) -> Vec<Generated> {
        let opening = parse_opening(opening).unwrap();
        let g = generate(xml, SRC, opening, TODAY, NOW).unwrap();
        assert_clean(&g);
        g
    }

    fn tx(e2e: &str, amount: &str) -> String {
        ct_tx(e2e, amount, "EUR", "Supplier One", CP_1)
    }

    fn single_day_xml() -> String {
        pain001(
            NS_001_09,
            &[ct_block(
                "P1",
                OWN_A,
                "<Dt>2026-12-01</Dt>",
                "",
                "SEPA",
                &[tx("E1", "100.00"), tx("E2", "25.50")],
            )],
        )
    }

    fn days_chain_xml() -> String {
        pain001(
            NS_001_03,
            &[
                ct_block("P2", OWN_A, "2026-12-03", "", "SEPA", &[tx("E2", "50.00")]),
                ct_block("P1", OWN_A, "2026-12-01", "", "SEPA", &[tx("E1", "100.00")]),
            ],
        )
    }

    fn multi_account_xml() -> String {
        pain001(
            NS_001_03,
            &[
                ct_block("PB", OWN_B, "2026-12-01", "", "SEPA", &[tx("E1", "10.00")]),
                ct_block("PA", OWN_A, "2026-12-01", "", "SEPA", &[tx("E2", "20.00")]),
            ],
        )
    }

    fn direct_debit_xml(lcl: &str) -> String {
        pain008(&[dd_block(
            "DD-1",
            OWN_B,
            "2026-12-02",
            lcl,
            &[
                dd_tx("E2E-D1", "30.00", "Member One", CP_2),
                dd_tx("E2E-D2", "20.00", "Member Two", CP_3),
            ],
        )])
    }

    fn escape_xml() -> String {
        pain001(
            NS_001_03,
            &[ct_block(
                "P1",
                OWN_A,
                "2026-12-01",
                "",
                "SEPA",
                &[ct_tx("E1", "1.00", "EUR", "A &amp; B &lt;Test&gt;", CP_1)],
            )],
        )
    }

    #[test]
    fn single_day_balances() {
        let g = gen(&single_day_xml(), "10000.00");
        assert_eq!(g.len(), 1);
        assert_eq!(
            (g[0].iban.as_str(), g[0].date.as_str()),
            (OWN_A, "2026-12-01")
        );
        let x = &g[0].xml;
        assert_eq!(
            texts(x, &["Bal", "Tp", "CdOrPrtry", "Cd"]),
            ["OPBD", "CLBD"]
        );
        assert_eq!(texts(x, &["Bal", "Amt"]), ["10000.00", "9874.50"]);
        assert_eq!(texts(x, &["Bal", "CdtDbtInd"]), ["CRDT", "CRDT"]);
        assert_eq!(texts(x, &["Stmt", "Ntry", "Amt"]), ["125.50"]);
        assert_eq!(texts(x, &["Stmt", "Ntry", "CdtDbtInd"]), ["DBIT"]);
        assert_eq!(texts(x, &["Btch", "NbOfTxs"]), ["2"]);
        assert_eq!(texts(x, &["TtlDbtNtries", "NbOfNtries"]), ["1"]);
        assert_eq!(texts(x, &["TxDtls", "Refs", "EndToEndId"]), ["E1", "E2"]);
        assert_eq!(
            texts(x, &["Cdtr", "Pty", "Nm"]),
            ["Supplier One", "Supplier One"]
        );
        assert_eq!(texts(x, &["Acct", "Id", "IBAN"]), [OWN_A]);
        assert_eq!(texts(x, &["Acct", "Ccy"]), ["EUR"]);
        assert_eq!(texts(x, &["Fmly", "SubFmlyCd"]), ["ESCT"]);
    }

    #[test]
    fn days_chain() {
        let g = gen(&days_chain_xml(), "10000.00");
        assert_eq!(
            g.iter().map(|g| g.date.as_str()).collect::<Vec<_>>(),
            ["2026-12-01", "2026-12-03"]
        );
        assert_eq!(texts(&g[0].xml, &["Bal", "Amt"]), ["10000.00", "9900.00"]);
        assert_eq!(texts(&g[1].xml, &["Bal", "Amt"]), ["9900.00", "9850.00"]);
        assert_eq!(texts(&g[0].xml, &["ElctrncSeqNb"]), ["1"]);
        assert_eq!(texts(&g[1].xml, &["ElctrncSeqNb"]), ["2"]);
    }

    #[test]
    fn same_account_same_day_merges() {
        let xml = pain001(
            NS_001_03,
            &[
                ct_block("P1", OWN_A, "2026-12-01", "", "SEPA", &[tx("E1", "1.00")]),
                ct_block("P2", OWN_A, "2026-12-01", "", "SEPA", &[tx("E2", "2.00")]),
            ],
        );
        let g = gen(&xml, "10000.00");
        assert_eq!(g.len(), 1);
        assert_eq!(texts(&g[0].xml, &["Stmt", "Ntry", "Amt"]), ["1.00", "2.00"]);
    }

    #[test]
    fn multi_account_separate_chains() {
        let g = gen(&multi_account_xml(), "10000.00");
        assert_eq!(
            g.iter().map(|g| g.iban.as_str()).collect::<Vec<_>>(),
            [OWN_B, OWN_A]
        );
        assert_eq!(texts(&g[0].xml, &["Bal", "Amt"]), ["10000.00", "9990.00"]);
        assert_eq!(texts(&g[1].xml, &["Bal", "Amt"]), ["10000.00", "9980.00"]);
        assert_eq!(texts(&g[1].xml, &["ElctrncSeqNb"]), ["1"]);
    }

    #[test]
    fn no_batch_one_entry_per_tx() {
        let xml = pain001(
            NS_001_03,
            &[ct_block(
                "P1",
                OWN_A,
                "2026-12-01",
                "false",
                "SEPA",
                &[tx("E1", "1.00"), tx("E2", "2.00"), tx("E3", "3.00")],
            )],
        );
        let g = gen(&xml, "10000.00");
        assert_eq!(
            texts(&g[0].xml, &["Stmt", "Ntry", "Amt"]),
            ["1.00", "2.00", "3.00"]
        );
        assert!(texts(&g[0].xml, &["Btch", "NbOfTxs"]).is_empty());
    }

    #[test]
    fn direct_debit_credits() {
        let g = gen(&direct_debit_xml("CORE"), "10000.00");
        let x = &g[0].xml;
        assert_eq!(texts(x, &["Stmt", "Ntry", "CdtDbtInd"]), ["CRDT"]);
        assert_eq!(texts(x, &["Bal", "Amt"]), ["10000.00", "10050.00"]);
        assert_eq!(texts(x, &["Fmly", "Cd"]), ["IDDT"]);
        assert_eq!(texts(x, &["Fmly", "SubFmlyCd"]), ["ESDD"]);
        assert_eq!(
            texts(x, &["Refs", "MndtId"]),
            ["MANDATE-E2E-D1", "MANDATE-E2E-D2"]
        );
        assert_eq!(
            texts(x, &["Dbtr", "Pty", "Nm"]),
            ["Member One", "Member Two"]
        );
        assert_eq!(texts(x, &["DbtrAcct", "Id", "IBAN"]), [CP_2, CP_3]);
        assert_eq!(texts(x, &["CdtrAcct", "Id", "IBAN"]), [OWN_B, OWN_B]);
        assert_eq!(
            texts(x, &["Cdtr", "Pty", "Id", "PrvtId", "Othr", "Id"]),
            ["DE98ZZZ09999999999", "DE98ZZZ09999999999"]
        );

        let b2b = gen(&direct_debit_xml("B2B"), "10000.00");
        assert_eq!(texts(&b2b[0].xml, &["Fmly", "SubFmlyCd"]), ["BBDD"]);
    }

    #[test]
    fn same_last_four_accounts_get_distinct_tags() {
        // Both end in 0001: the tag grows until the accounts differ.
        let (a, b) = ("DE57000000000000010001", "DE08000000000000020001");
        let xml = pain001(
            NS_001_03,
            &[
                ct_block("PA", a, "2026-12-01", "", "SEPA", &[tx("E1", "1.00")]),
                ct_block("PB", b, "2026-12-01", "", "SEPA", &[tx("E2", "2.00")]),
            ],
        );
        let g = gen(&xml, "10000.00");
        assert_eq!(
            g.iter().map(|g| g.tag.as_str()).collect::<Vec<_>>(),
            ["10001", "20001"]
        );
        assert_ne!(
            texts(&g[0].xml, &["Stmt", "Id"]),
            texts(&g[1].xml, &["Stmt", "Id"])
        );
        assert!(texts(&g[1].xml, &["Stmt", "Id"])[0].contains("-20001-"));

        let single = gen(&single_day_xml(), "10000.00");
        assert_eq!(single[0].tag, "0001");
    }

    #[test]
    fn transaction_level_payment_type() {
        // SvcLvl on the transaction instead of the PmtInf (some ERP exports).
        let ct = pain001(
            NS_001_03,
            &[ct_block(
                "P1",
                OWN_A,
                "2026-12-01",
                "",
                "",
                &[tx("E1", "1.00")],
            )],
        )
        .replace(
            "</PmtId><Amt>",
            "</PmtId><PmtTpInf><SvcLvl><Cd>SEPA</Cd></SvcLvl></PmtTpInf><Amt>",
        );
        assert_eq!(
            texts(&gen(&ct, "10000.00")[0].xml, &["Fmly", "SubFmlyCd"]),
            ["ESCT"]
        );
        // LclInstrm B2B on the transaction, none on the PmtInf.
        let dd = direct_debit_xml("CORE")
            .replace("<LclInstrm><Cd>CORE</Cd></LclInstrm>", "")
            .replace(
                "</PmtId><InstdAmt",
                "</PmtId><PmtTpInf><LclInstrm><Cd>B2B</Cd></LclInstrm></PmtTpInf><InstdAmt",
            );
        assert_eq!(
            texts(&gen(&dd, "10000.00")[0].xml, &["Fmly", "SubFmlyCd"]),
            ["BBDD"]
        );
    }

    #[test]
    fn non_sepa_is_xbct() {
        let xml = pain001(
            NS_001_03,
            &[ct_block(
                "P1",
                OWN_A,
                "2026-12-01",
                "",
                "URGP",
                &[tx("E1", "1.00")],
            )],
        );
        assert_eq!(
            texts(&gen(&xml, "10000.00")[0].xml, &["Fmly", "SubFmlyCd"]),
            ["XBCT"]
        );
    }

    #[test]
    fn negative_opening_chains() {
        let xml = pain001(
            NS_001_03,
            &[ct_block(
                "P1",
                OWN_A,
                "2026-12-01",
                "",
                "SEPA",
                &[tx("E1", "10.00")],
            )],
        );
        let g = gen(&xml, "-50.00");
        assert_eq!(texts(&g[0].xml, &["Bal", "Amt"]), ["50.00", "60.00"]);
        assert_eq!(texts(&g[0].xml, &["Bal", "CdtDbtInd"]), ["DBIT", "DBIT"]);
        let g = gen(&xml, "5.00");
        assert_eq!(texts(&g[0].xml, &["Bal", "Amt"]), ["5.00", "5.00"]);
        assert_eq!(texts(&g[0].xml, &["Bal", "CdtDbtInd"]), ["CRDT", "DBIT"]);
    }

    #[test]
    fn escapes_copied_text() {
        let g = gen(&escape_xml(), "10000.00");
        assert!(g[0].xml.contains("A &amp; B &lt;Test&gt;"), "{}", g[0].xml);
        assert_eq!(texts(&g[0].xml, &["Cdtr", "Pty", "Nm"]), ["A & B <Test>"]);
    }

    #[test]
    fn synthetic_markers() {
        let g = gen(&days_chain_xml(), "10000.00");
        for x in g.iter().map(|g| &g.xml) {
            let ids = [
                texts(x, &["GrpHdr", "MsgId"]),
                texts(x, &["Stmt", "Id"]),
                texts(x, &["Ntry", "AcctSvcrRef"]),
            ]
            .concat();
            assert_eq!(ids.len(), 3);
            assert!(
                ids.iter().all(|i| i.starts_with("SYNTH-") && i.len() <= 35),
                "{ids:?}"
            );
            assert!(x.contains("SYNTHETIC TEST BANK"));
            assert!(texts(x, &["AddtlStmtInf"])[0].contains(SRC));
            assert!(texts(x, &["AddtlNtryInf"])[0].contains(SRC));
        }
        assert_ne!(
            texts(&g[0].xml, &["GrpHdr", "MsgId"]),
            texts(&g[1].xml, &["GrpHdr", "MsgId"])
        );
        assert_ne!(
            texts(&g[0].xml, &["Stmt", "Id"]),
            texts(&g[1].xml, &["Stmt", "Id"])
        );
    }

    #[test]
    fn refused_file_generates_nothing() {
        let xml = pain001(
            NS_001_03,
            &[ct_block(
                "P1",
                OWN_A,
                "2026-12-01",
                "",
                "",
                &[ct_tx("E", "1.00", "USD", "S", CP_1)],
            )],
        );
        assert!(generate(&xml, SRC, eur("1.00"), TODAY, NOW).is_err());
    }

    #[test]
    fn preview_counts() {
        let xml = pain001(
            NS_001_03,
            &[
                ct_block(
                    "P1",
                    OWN_A,
                    "2026-12-01",
                    "",
                    "",
                    &[tx("E1", "1.00"), tx("E2", "1.00")],
                ),
                ct_block("P2", OWN_A, "2026-12-03", "", "", &[tx("E3", "1.00")]),
                ct_block(
                    "P3",
                    OWN_B,
                    "2026-12-01",
                    "",
                    "",
                    &[tx("E4", "1.00"), tx("E5", "1.00")],
                ),
                ct_block(
                    "P4",
                    OWN_B,
                    "2026-12-05",
                    "",
                    "",
                    &[tx("E6", "1.00"), tx("E7", "1.00")],
                ),
            ],
        );
        assert_eq!(
            preview(&xml, TODAY),
            StatementPreview {
                kind: "pain.001".into(),
                accounts: 2,
                days: 3,
                statements: 4,
                transactions: 7,
                problems: vec![],
            }
        );
        assert_eq!(preview(&direct_debit_xml("CORE"), TODAY).kind, "pain.008");
        let usd = pain001(
            NS_001_03,
            &[ct_block(
                "P1",
                OWN_A,
                "2026-12-01",
                "",
                "",
                &[ct_tx("E", "1.00", "USD", "S", CP_1)],
            )],
        );
        let p = preview(&usd, TODAY);
        assert_eq!(
            (
                p.accounts,
                p.days,
                p.statements,
                p.transactions,
                p.problems.len()
            ),
            (0, 0, 0, 0, 1)
        );
    }

    #[test]
    fn now_utc_shape() {
        let n = now_utc();
        assert_eq!(n.len(), 19, "{n}");
        assert_eq!(&n[10..11], "T");
        assert!(n[..10] >= *"2026-01-01");
    }

    /// The local schema folder with camt.053.001.08.xsd, if any (never in CI).
    fn schema_dir() -> Option<std::path::PathBuf> {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        if let Ok(d) = std::env::var("SEPA_SCHEMA_DIR") {
            dirs.push(d.into());
        }
        if let Ok(d) = std::env::var("APPDATA") {
            dirs.push(
                std::path::Path::new(&d)
                    .join("dev.sepa.validator")
                    .join("schemas"),
            );
        }
        dirs.push(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../xml_schema"));
        dirs.into_iter()
            .find(|d| d.join("camt.053.001.08.xsd").exists())
    }

    #[test]
    fn validates_against_xsd() {
        let Some(dir) = schema_dir() else {
            eprintln!("SKIP: camt.053.001.08.xsd not found (set SEPA_SCHEMA_DIR)");
            return;
        };
        let mut validator = crate::validator::Validator::new(dir);
        let inputs = [
            single_day_xml(),
            days_chain_xml(),
            multi_account_xml(),
            direct_debit_xml("CORE"),
            escape_xml(),
        ];
        for (i, input) in inputs.iter().enumerate() {
            for (j, g) in gen(input, "10000.00").iter().enumerate() {
                let path = std::env::temp_dir().join(format!(
                    "sepa-statement-gen-{}-{i}-{j}.xml",
                    std::process::id()
                ));
                std::fs::write(&path, &g.xml).unwrap();
                let r = validator.validate_file(&path);
                std::fs::remove_file(&path).ok();
                assert!(r.namespace.ends_with("camt.053.001.08"), "{}", r.namespace);
                assert_eq!(
                    (r.errors, r.warnings),
                    (0, 0),
                    "{:?}\n{}",
                    r.messages,
                    g.xml
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) const TODAY: &str = "2026-11-07";
    pub(super) const NS_001_03: &str = "urn:iso:std:iso:20022:tech:xsd:pain.001.001.03";
    pub(super) const NS_001_09: &str = "urn:iso:std:iso:20022:tech:xsd:pain.001.001.09";
    pub(super) const NS_008_08: &str = "urn:iso:std:iso:20022:tech:xsd:pain.008.001.08";
    // Synthetic IBANs (bank code 00000000), valid check digits.
    pub(super) const OWN_A: &str = "DE09000000000000000001";
    pub(super) const OWN_B: &str = "DE79000000000000000002";
    pub(super) const CP_1: &str = "DE52000000000000000003";
    pub(super) const CP_2: &str = "DE25000000000000000004";
    pub(super) const CP_3: &str = "DE95000000000000000005";
    const BAD_IBAN: &str = "DE00000000000000000003";

    /// A credit transfer transaction.
    pub(super) fn ct_tx(e2e: &str, amount: &str, ccy: &str, name: &str, iban: &str) -> String {
        format!(
            "<CdtTrfTxInf><PmtId><InstrId>I-{e2e}</InstrId><EndToEndId>{e2e}</EndToEndId></PmtId>\
             <Amt><InstdAmt Ccy=\"{ccy}\">{amount}</InstdAmt></Amt>\
             <Cdtr><Nm>{name}</Nm></Cdtr><CdtrAcct><Id><IBAN>{iban}</IBAN></Id></CdtrAcct>\
             <RmtInf><Ustrd>Invoice {e2e}</Ustrd></RmtInf></CdtTrfTxInf>"
        )
    }

    /// A pain.001 PmtInf. `date` is the inner XML of `ReqdExctnDt`;
    /// `btch` the `BtchBookg` value or "" to leave it out; `svc` the SvcLvl code or "".
    pub(super) fn ct_block(
        id: &str,
        iban: &str,
        date: &str,
        btch: &str,
        svc: &str,
        txs: &[String],
    ) -> String {
        let btch = if btch.is_empty() {
            String::new()
        } else {
            format!("<BtchBookg>{btch}</BtchBookg>")
        };
        let svc = if svc.is_empty() {
            String::new()
        } else {
            format!("<PmtTpInf><SvcLvl><Cd>{svc}</Cd></SvcLvl></PmtTpInf>")
        };
        format!(
            "<PmtInf><PmtInfId>{id}</PmtInfId><PmtMtd>TRF</PmtMtd>{btch}{svc}\
             <ReqdExctnDt>{date}</ReqdExctnDt><Dbtr><Nm>Example Owner Ltd</Nm></Dbtr>\
             <DbtrAcct><Id><IBAN>{iban}</IBAN></Id></DbtrAcct>\
             <DbtrAgt><FinInstnId><BICFI>TESTDEFFXXX</BICFI></FinInstnId></DbtrAgt>{}</PmtInf>",
            txs.concat()
        )
    }

    pub(super) fn pain001(ns: &str, blocks: &[String]) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Document xmlns=\"{ns}\"><CstmrCdtTrfInitn>\
             <GrpHdr><MsgId>MSG-TEST-1</MsgId></GrpHdr>{}</CstmrCdtTrfInitn></Document>\n",
            blocks.concat()
        )
    }

    pub(super) fn dd_tx(e2e: &str, amount: &str, name: &str, iban: &str) -> String {
        format!(
            "<DrctDbtTxInf><PmtId><EndToEndId>{e2e}</EndToEndId></PmtId>\
             <InstdAmt Ccy=\"EUR\">{amount}</InstdAmt>\
             <DrctDbtTx><MndtRltdInf><MndtId>MANDATE-{e2e}</MndtId><DtOfSgntr>2025-01-01</DtOfSgntr></MndtRltdInf></DrctDbtTx>\
             <Dbtr><Nm>{name}</Nm></Dbtr><DbtrAcct><Id><IBAN>{iban}</IBAN></Id></DbtrAcct>\
             <RmtInf><Ustrd>Membership {e2e}</Ustrd></RmtInf></DrctDbtTxInf>"
        )
    }

    /// A pain.008 PmtInf; `lcl` is the LclInstrm code (CORE/B2B).
    pub(super) fn dd_block(id: &str, iban: &str, date: &str, lcl: &str, txs: &[String]) -> String {
        format!(
            "<PmtInf><PmtInfId>{id}</PmtInfId><PmtMtd>DD</PmtMtd>\
             <PmtTpInf><SvcLvl><Cd>SEPA</Cd></SvcLvl><LclInstrm><Cd>{lcl}</Cd></LclInstrm><SeqTp>RCUR</SeqTp></PmtTpInf>\
             <ReqdColltnDt>{date}</ReqdColltnDt><Cdtr><Nm>Example Club e.V.</Nm></Cdtr>\
             <CdtrAcct><Id><IBAN>{iban}</IBAN></Id></CdtrAcct>\
             <CdtrAgt><FinInstnId><BICFI>TESTDEFFXXX</BICFI></FinInstnId></CdtrAgt>\
             <CdtrSchmeId><Id><PrvtId><Othr><Id>DE98ZZZ09999999999</Id><SchmeNm><Prtry>SEPA</Prtry></SchmeNm></Othr></PrvtId></Id></CdtrSchmeId>{}</PmtInf>",
            txs.concat()
        )
    }

    pub(super) fn pain008(blocks: &[String]) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Document xmlns=\"{NS_008_08}\"><CstmrDrctDbtInitn>\
             <GrpHdr><MsgId>MSG-TEST-DD</MsgId></GrpHdr>{}</CstmrDrctDbtInitn></Document>\n",
            blocks.concat()
        )
    }

    fn amt(s: &str) -> Amount {
        Amount::parse(s).unwrap()
    }

    #[test]
    fn iban_fixtures_are_valid() {
        for iban in [OWN_A, OWN_B, CP_1, CP_2, CP_3] {
            assert!(iban_is_valid(iban), "{iban}");
        }
        assert!(!iban_is_valid(BAD_IBAN));
    }

    #[test]
    fn parses_credit_transfer() {
        let xml = pain001(
            NS_001_09,
            &[ct_block(
                "PMT-1",
                OWN_A,
                "<Dt>2026-12-01</Dt>",
                "",
                "SEPA",
                &[ct_tx("E2E-1", "100.00", "EUR", "Supplier One", CP_1)],
            )],
        );
        let p = parse_payment(&xml, TODAY).unwrap();
        assert_eq!(p.kind, Kind::CreditTransfer);
        assert_eq!(p.msg_id.as_deref(), Some("MSG-TEST-1"));
        assert_eq!(p.blocks.len(), 1);
        let b = &p.blocks[0];
        assert_eq!(b.pmt_inf_id.as_deref(), Some("PMT-1"));
        assert_eq!(b.iban, OWN_A);
        assert_eq!(b.owner.as_deref(), Some("Example Owner Ltd"));
        assert_eq!(b.bic.as_deref(), Some("TESTDEFFXXX"));
        assert_eq!(
            b.txs,
            vec![Tx {
                instr_id: Some("I-E2E-1".into()),
                end_to_end_id: Some("E2E-1".into()),
                amount: amt("100.00"),
                name: Some("Supplier One".into()),
                iban: Some(CP_1.into()),
                ustrd: vec!["Invoice E2E-1".into()],
                mandate_id: None,
                sepa: true,
                b2b: false,
            }]
        );
    }

    #[test]
    fn bic_of_version_03() {
        let xml = pain001(
            NS_001_03,
            &[ct_block(
                "P",
                OWN_A,
                "2026-12-01",
                "",
                "",
                &[ct_tx("E", "1.00", "EUR", "S", CP_1)],
            )],
        )
        .replace("BICFI", "BIC");
        assert_eq!(
            parse_payment(&xml, TODAY).unwrap().blocks[0].bic.as_deref(),
            Some("TESTDEFFXXX")
        );
    }

    #[test]
    fn parses_direct_debit() {
        let xml = pain008(&[dd_block(
            "DD-1",
            OWN_B,
            "2026-12-02",
            "CORE",
            &[dd_tx("E2E-D1", "30.00", "Member One", CP_2)],
        )]);
        let p = parse_payment(&xml, TODAY).unwrap();
        assert_eq!(p.kind, Kind::DirectDebit);
        let b = &p.blocks[0];
        assert_eq!(b.iban, OWN_B);
        assert_eq!(b.owner.as_deref(), Some("Example Club e.V."));
        assert_eq!(b.creditor_id.as_deref(), Some("DE98ZZZ09999999999"));
        assert_eq!(b.date, "2026-12-02");
        assert!(b.sepa);
        assert!(!b.b2b);
        let t = &b.txs[0];
        assert_eq!(t.name.as_deref(), Some("Member One"));
        assert_eq!(t.iban.as_deref(), Some(CP_2));
        assert_eq!(t.mandate_id.as_deref(), Some("MANDATE-E2E-D1"));
        assert_eq!(t.amount, amt("30.00"));

        let b2b = pain008(&[dd_block(
            "DD-2",
            OWN_B,
            "2026-12-02",
            "B2B",
            &[dd_tx("E", "1.00", "M", CP_2)],
        )]);
        assert!(parse_payment(&b2b, TODAY).unwrap().blocks[0].b2b);
    }

    #[test]
    fn creditor_id_at_transaction_level() {
        let xml = pain008(&[dd_block("DD-1", OWN_B, "2026-12-02", "CORE", &[dd_tx("E", "1.00", "M", CP_2)])])
            .replace(
                "<CdtrSchmeId><Id><PrvtId><Othr><Id>DE98ZZZ09999999999</Id><SchmeNm><Prtry>SEPA</Prtry></SchmeNm></Othr></PrvtId></Id></CdtrSchmeId>",
                "",
            )
            .replace(
                "</MndtRltdInf>",
                "</MndtRltdInf><CdtrSchmeId><Id><PrvtId><Othr><Id>DE11ZZZ00000000001</Id></Othr></PrvtId></Id></CdtrSchmeId>",
            );
        assert_eq!(
            parse_payment(&xml, TODAY).unwrap().blocks[0]
                .creditor_id
                .as_deref(),
            Some("DE11ZZZ00000000001")
        );
    }

    #[test]
    fn date_forms_03_and_09() {
        for (ns, date) in [
            (NS_001_03, "2026-12-01"),
            (NS_001_09, "<Dt>2026-12-01</Dt>"),
            (NS_001_09, "<DtTm>2026-12-01T10:00:00</DtTm>"),
        ] {
            let xml = pain001(
                ns,
                &[ct_block(
                    "P",
                    OWN_A,
                    date,
                    "",
                    "",
                    &[ct_tx("E", "1.00", "EUR", "S", CP_1)],
                )],
            );
            assert_eq!(
                parse_payment(&xml, TODAY).unwrap().blocks[0].date,
                "2026-12-01",
                "{date}"
            );
        }
    }

    #[test]
    fn past_and_asap_dates_become_today() {
        for date in ["2026-01-05", "1999-01-01"] {
            let xml = pain001(
                NS_001_03,
                &[ct_block(
                    "P",
                    OWN_A,
                    date,
                    "",
                    "",
                    &[ct_tx("E", "1.00", "EUR", "S", CP_1)],
                )],
            );
            assert_eq!(
                parse_payment(&xml, TODAY).unwrap().blocks[0].date,
                TODAY,
                "{date}"
            );
        }
    }

    #[test]
    fn batch_flag() {
        for (btch, expected) in [("false", false), ("true", true), ("", true)] {
            let xml = pain001(
                NS_001_03,
                &[ct_block(
                    "P",
                    OWN_A,
                    "2026-12-01",
                    btch,
                    "",
                    &[ct_tx("E", "1.00", "EUR", "S", CP_1)],
                )],
            );
            assert_eq!(
                parse_payment(&xml, TODAY).unwrap().blocks[0].batch,
                expected,
                "{btch:?}"
            );
        }
    }

    #[test]
    fn sepa_flag() {
        for (svc, expected) in [("SEPA", true), ("URGP", false), ("", false)] {
            let xml = pain001(
                NS_001_03,
                &[ct_block(
                    "P",
                    OWN_A,
                    "2026-12-01",
                    "",
                    svc,
                    &[ct_tx("E", "1.00", "EUR", "S", CP_1)],
                )],
            );
            assert_eq!(
                parse_payment(&xml, TODAY).unwrap().blocks[0].sepa,
                expected,
                "{svc:?}"
            );
        }
    }

    #[test]
    fn refuses_with_reasons() {
        let other_account = ct_block(
            "P2",
            OWN_B,
            "2026-12-01",
            "",
            "",
            &[ct_tx("E3", "5.00", "EUR", "S", CP_1)],
        )
        .replace(
            &format!("<IBAN>{OWN_B}</IBAN>"),
            "<Othr><Id>12345</Id></Othr>",
        );
        let xml = pain001(
            NS_001_03,
            &[
                ct_block(
                    "P1",
                    OWN_A,
                    "2026-12-01",
                    "",
                    "",
                    &[
                        ct_tx("E1", "10.00", "USD", "S", CP_1),
                        ct_tx("E2", "12,50", "EUR", "S", CP_1),
                    ],
                ),
                other_account,
            ],
        );
        let reasons = parse_payment(&xml, TODAY).unwrap_err();
        assert_eq!(reasons.len(), 3, "{reasons:?}");
        assert!(reasons[0].contains("USD"), "{}", reasons[0]);
        assert!(reasons[1].contains("12,50"), "{}", reasons[1]);
        assert!(reasons[2].contains("P2"), "{}", reasons[2]);

        let empty = pain001(
            NS_001_03,
            &[ct_block("P", OWN_A, "2026-12-01", "", "", &[])],
        );
        assert_eq!(
            parse_payment(&empty, TODAY).unwrap_err(),
            vec!["The file contains no transactions.".to_string()]
        );

        let camt = "<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:camt.053.001.08\"><BkToCstmrStmt/></Document>";
        let err = parse_payment(camt, TODAY).unwrap_err();
        assert!(err[0].contains("pain.001 or pain.008"), "{err:?}");

        let bad_date = pain001(
            NS_001_03,
            &[ct_block(
                "P",
                OWN_A,
                "next week",
                "",
                "",
                &[ct_tx("E", "1.00", "EUR", "S", CP_1)],
            )],
        );
        assert!(parse_payment(&bad_date, TODAY).unwrap_err()[0].contains("date"));

        let fx = pain001(
            NS_001_09,
            &[ct_block(
                "P",
                OWN_A,
                "<Dt>2026-12-01</Dt>",
                "",
                "",
                &[ct_tx("E", "1.00", "EUR", "S", CP_1)],
            )],
        )
        .replace(
            "<InstdAmt Ccy=\"EUR\">1.00</InstdAmt>",
            "<EqvtAmt><Amt Ccy=\"EUR\">1.00</Amt><CcyOfTrf>USD</CcyOfTrf></EqvtAmt>",
        );
        assert!(parse_payment(&fx, TODAY).unwrap_err()[0].contains("EqvtAmt"));
    }

    #[test]
    fn refuses_bad_iban() {
        let xml = pain001(
            NS_001_03,
            &[ct_block(
                "P",
                OWN_A,
                "2026-12-01",
                "",
                "",
                &[ct_tx("E", "1.00", "EUR", "S", BAD_IBAN)],
            )],
        );
        let err = parse_payment(&xml, TODAY).unwrap_err();
        assert!(err[0].contains(BAD_IBAN), "{err:?}");
    }

    #[test]
    fn malformed_xml_is_refused() {
        let err =
            parse_payment("<Document><CstmrCdtTrfInitn><GrpHdr></Document>", TODAY).unwrap_err();
        assert_eq!(err.len(), 1, "{err:?}");
    }

    #[test]
    fn parse_opening_forms() {
        assert_eq!(parse_opening("10000.00"), Ok(Amount(1_000_000_000)));
        assert_eq!(parse_opening("10,000.00"), Ok(Amount(1_000_000_000)));
        assert_eq!(parse_opening(" 10000 "), Ok(Amount(1_000_000_000)));
        assert_eq!(parse_opening("1,234,567.89"), Ok(Amount(123_456_789_000)));
        assert_eq!(parse_opening("-250.5"), Ok(Amount(-25_050_000)));
        for bad in ["10.000,00", "12,50", "abc", "", "1.234", "-", "1,00.00"] {
            assert!(parse_opening(bad).is_err(), "{bad:?}");
        }
    }
}
