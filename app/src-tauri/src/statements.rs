//! Bank-to-customer cash management messages: account report (camt.052),
//! statement (camt.053) and debit/credit notification (camt.054). One parse
//! yields the statement blocks (`Rpt`/`Stmt`/`Ntfctn`) with their balances,
//! transaction summary and entries; from that come the plausibility warnings
//! (`check`) and the Overview/Transactions data (`summarize`).

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde::Serialize;

use crate::model::Message;
use crate::payments::{Detail, RemittanceEntry};
use crate::plausibility::{warning, Amount, LineCounter};

/// Root elements of the camt messages handled here.
const ROOTS: &[&str] = &["BkToCstmrAcctRpt", "BkToCstmrStmt", "BkToCstmrDbtCdtNtfctn"];
/// Statement-level blocks below the root.
const BLOCKS: &[&str] = &["Rpt", "Stmt", "Ntfctn"];

/// A value with the (formatted-text) line it was read from.
#[derive(Clone, Debug, Default)]
struct At {
    value: String,
    line: u32,
}

#[derive(Debug, Default)]
struct Balance {
    code: Option<String>,
    amount: Option<At>,
    currency: Option<String>,
    debit: bool,
    date: Option<String>,
}

impl Balance {
    fn signed(&self) -> Option<Amount> {
        let a = Amount::parse(&self.amount.as_ref()?.value)?;
        Some(if self.debit { Amount(-a.0) } else { a })
    }
}

#[derive(Debug, Default)]
struct Entry {
    amount: Option<String>,
    currency: Option<String>,
    /// `CdtDbtInd`: Some(true) for DBIT, Some(false) for CRDT.
    debit: Option<bool>,
    status: Option<String>,
    booking_date: Option<String>,
    end_to_end_id: Option<String>,
    ustrd: Vec<String>,
    debtor_name: Option<String>,
    debtor_iban: Option<String>,
    creditor_name: Option<String>,
    creditor_iban: Option<String>,
    additional_info: Option<String>,
    /// `BkTxCd`: ISO domain, family and sub-family codes, and the proprietary
    /// code (DK: the GVC, e.g. "166" or "NTRF+166"). Entry level wins over
    /// the first transaction's codes.
    domain: Option<String>,
    family: Option<String>,
    sub_family: Option<String>,
    proprietary: Option<String>,
    /// Number of `TxDtls` (more than one: a batch; details come from the first).
    tx_count: u32,
    /// Currency of the last amount element opened inside the entry.
    last_ccy: Option<String>,
    /// Further entry information for the details view, first value per label.
    details: Vec<(&'static str, String)>,
}

impl Entry {
    /// Records `value` under `label` unless the label already has a value.
    fn detail(&mut self, label: &'static str, value: &str) {
        if !self.details.iter().any(|(l, _)| *l == label) {
            self.details.push((label, value.to_string()));
        }
    }

    /// An amount with the currency of its element ("12.50 USD").
    fn detail_amount(&mut self, label: &'static str, value: &str) {
        let v = match &self.last_ccy {
            Some(c) => format!("{value} {c}"),
            None => value.to_string(),
        };
        self.detail(label, &v);
    }

    /// "PMNT-RCDT-ESCT", or as much of it as the entry has.
    fn iso_code(&self) -> Option<String> {
        let parts: Vec<&str> = [&self.domain, &self.family, &self.sub_family]
            .into_iter()
            .map_while(|p| p.as_deref())
            .collect();
        (!parts.is_empty()).then(|| parts.join("-"))
    }

    /// The German business transaction code (GVC): a three-digit code given
    /// alone ("166") or as the second part of a SWIFT-style code ("NTRF+166++901").
    fn gvc(&self) -> Option<String> {
        let p = self.proprietary.as_deref()?.trim();
        let candidate = if p.contains('+') {
            p.split('+').nth(1)?
        } else {
            p
        };
        (candidate.len() == 3 && candidate.bytes().all(|b| b.is_ascii_digit()))
            .then(|| candidate.to_string())
    }
}

impl Entry {
    /// Booked entries move the booked balance; pending/info entries don't.
    /// Without a status (older versions) an entry counts as booked.
    fn booked(&self) -> bool {
        self.status.as_deref().is_none_or(|s| s == "BOOK")
    }
}

/// Declared values of `TxsSummry`.
#[derive(Debug, Default)]
struct TxsSummary {
    nb: Option<At>,
    sum: Option<At>,
    net_amount: Option<At>,
    net_debit: bool,
    credit_nb: Option<At>,
    credit_sum: Option<At>,
    debit_nb: Option<At>,
    debit_sum: Option<At>,
}

#[derive(Debug, Default)]
struct Block {
    kind: String,
    id: Option<String>,
    electronic_seq_nb: Option<String>,
    legal_seq_nb: Option<String>,
    from: Option<String>,
    to: Option<String>,
    iban: Option<String>,
    currency: Option<String>,
    owner: Option<String>,
    bic: Option<String>,
    balances: Vec<Balance>,
    summary: TxsSummary,
    entries: Vec<Entry>,
}

fn attr(e: &BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

/// The statement blocks of a camt.052/053/054 document; empty for anything else.
fn parse(xml: &str) -> Vec<Block> {
    let mut reader = Reader::from_str(xml);
    let mut lines = LineCounter::new(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut block: Option<Block> = None;
    loop {
        let line = lines.at(reader.buffer_position() as usize);
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if stack.len() == 1 && !ROOTS.contains(&name.as_str()) {
                    return Vec::new();
                }
                if stack.len() == 2 && BLOCKS.contains(&name.as_str()) {
                    block = Some(Block {
                        kind: name.clone(),
                        ..Block::default()
                    });
                }
                if let Some(b) = block.as_mut() {
                    start(b, &stack, &name, &e);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                if stack.len() == 1 {
                    let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                    if !ROOTS.contains(&name.as_str()) {
                        return Vec::new();
                    }
                }
            }
            Ok(Event::End(_)) => {
                if stack.len() == 3 && BLOCKS.contains(&stack[2].as_str()) {
                    blocks.extend(block.take());
                }
                stack.pop();
            }
            Ok(ev @ (Event::Text(_) | Event::CData(_))) => {
                // CDATA is literal text (`<![CDATA[A & B]]>`), not escaped.
                let v = match &ev {
                    Event::Text(t) => t.unescape().ok().map(|v| v.into_owned()),
                    Event::CData(c) => Some(String::from_utf8_lossy(c).into_owned()),
                    _ => None,
                };
                if let (Some(b), Some(v)) = (block.as_mut(), v) {
                    let v = v.trim();
                    if !v.is_empty() {
                        text(b, &stack[3..], v, line);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    blocks
}

/// Opening tags inside a block: new balances/entries and currency attributes.
fn start(b: &mut Block, stack: &[String], name: &str, e: &BytesStart) {
    let path: Vec<&str> = stack.iter().skip(3).map(String::as_str).collect();
    match (path.as_slice(), name) {
        ([], "Bal") => b.balances.push(Balance::default()),
        ([], "Ntry") => b.entries.push(Entry::default()),
        (["Bal"], "Amt") => {
            if let Some(bal) = b.balances.last_mut() {
                bal.currency = attr(e, b"Ccy");
            }
        }
        (["Ntry", "NtryDtls"], "TxDtls") => {
            if let Some(n) = b.entries.last_mut() {
                n.tx_count += 1;
            }
        }
        (["Ntry", ..], "Amt" | "TtlAmt") if path.len() > 1 => {
            if let Some(n) = b.entries.last_mut() {
                n.last_ccy = attr(e, b"Ccy");
            }
        }
        (["Ntry"], "Amt") => {
            if let Some(n) = b.entries.last_mut() {
                n.currency = attr(e, b"Ccy");
            }
        }
        _ => {}
    }
}

/// Text inside a block; `path` is the element path below the block element.
/// Details of the first `TxDtls` of an entry.
fn tx_detail(n: &mut Entry, tx: &[&str], v: &str) {
    match tx {
        ["Refs", "EndToEndId"] if v != "NOTPROVIDED" => n.detail("EndToEndId", v),
        ["Refs", "MndtId"] => n.detail("Mandate ID", v),
        ["Refs", "InstrId"] => n.detail("InstrId", v),
        ["Refs", "PmtInfId"] => n.detail("PmtInfId", v),
        ["Refs", "TxId"] => n.detail("TxId", v),
        ["Refs", "UETR"] => n.detail("UETR", v),
        ["RltdPties", "Dbtr", "Pty", "Nm"] | ["RltdPties", "Dbtr", "Nm"] => n.detail("Debtor", v),
        ["RltdPties", "Cdtr", "Pty", "Nm"] | ["RltdPties", "Cdtr", "Nm"] => n.detail("Creditor", v),
        ["RltdPties", "UltmtDbtr", "Pty", "Nm"] | ["RltdPties", "UltmtDbtr", "Nm"] => {
            n.detail("Ultimate debtor", v)
        }
        ["RltdPties", "UltmtCdtr", "Pty", "Nm"] | ["RltdPties", "UltmtCdtr", "Nm"] => {
            n.detail("Ultimate creditor", v)
        }
        ["RltdPties", "Cdtr", "Pty", "Id", "PrvtId", "Othr", "Id"] => n.detail("Creditor ID", v),
        ["RltdPties", "DbtrAcct", "Id", "IBAN"] | ["RltdPties", "DbtrAcct", "Id", "Othr", "Id"] => {
            n.detail("Debtor account", v)
        }
        ["RltdPties", "CdtrAcct", "Id", "IBAN"] | ["RltdPties", "CdtrAcct", "Id", "Othr", "Id"] => {
            n.detail("Creditor account", v)
        }
        ["RltdAgts", "DbtrAgt", "FinInstnId", "BICFI" | "BIC"] => n.detail("Debtor bank (BIC)", v),
        ["RltdAgts", "CdtrAgt", "FinInstnId", "BICFI" | "BIC"] => {
            n.detail("Creditor bank (BIC)", v)
        }
        ["Purp", "Cd"] => n.detail("Purpose code", v),
        ["RmtInf", "Strd", "CdtrRefInf", "Ref"] => n.detail("Creditor reference", v),
        ["RtrInf", "Rsn", "Cd"] => n.detail("Return reason", v),
        ["RtrInf", "AddtlInf"] => n.detail("Return info", v),
        ["AmtDtls", "InstdAmt", "Amt"] => n.detail_amount("Instructed amount", v),
        ["AmtDtls", "TxAmt", "Amt"] => n.detail_amount("Transaction amount", v),
        ["AmtDtls", "CntrValAmt", "Amt"] => n.detail_amount("Counter value", v),
        ["AmtDtls", _, "CcyXchg", "XchgRate"] => n.detail("Exchange rate", v),
        ["Chrgs", "Rcrd", "Amt"] | ["Chrgs", "TtlChrgsAndTaxAmt"] => n.detail_amount("Charges", v),
        ["AddtlTxInf"] => n.detail("Additional info", v),
        _ => {}
    }
}

/// Display order of the details; labels not listed go last.
const DETAIL_ORDER: &[&str] = &[
    "Posting text",
    "Value date",
    "Reversal",
    "Debtor",
    "Debtor account",
    "Debtor bank (BIC)",
    "Ultimate debtor",
    "Creditor",
    "Creditor account",
    "Creditor bank (BIC)",
    "Ultimate creditor",
    "Creditor ID",
    "Mandate ID",
    "EndToEndId",
    "Creditor reference",
    "Purpose code",
    "Return reason",
    "Return info",
    "Instructed amount",
    "Transaction amount",
    "Counter value",
    "Exchange rate",
    "Charges",
    "Batch: transactions",
    "Batch: total",
    "Batch: PmtInfId",
    "Bank reference",
    "Entry reference",
    "InstrId",
    "PmtInfId",
    "TxId",
    "UETR",
    "Additional info",
];

fn text(b: &mut Block, path: &[String], v: &str, line: u32) {
    let p: Vec<&str> = path.iter().map(String::as_str).collect();
    let at = || At {
        value: v.to_string(),
        line,
    };
    let own = |o: &mut Option<String>| {
        o.get_or_insert_with(|| v.to_string());
    };
    match p.as_slice() {
        ["Id"] => own(&mut b.id),
        ["ElctrncSeqNb"] => own(&mut b.electronic_seq_nb),
        ["LglSeqNb"] => own(&mut b.legal_seq_nb),
        ["FrToDt", "FrDtTm" | "FrDt"] => own(&mut b.from),
        ["FrToDt", "ToDtTm" | "ToDt"] => own(&mut b.to),
        ["Acct", "Id", "IBAN"] => own(&mut b.iban),
        ["Acct", "Ccy"] => own(&mut b.currency),
        ["Acct", "Ownr", "Nm"] => own(&mut b.owner),
        ["Acct", "Svcr", "FinInstnId", "BICFI" | "BIC"] => own(&mut b.bic),
        ["Bal", rest @ ..] => {
            let Some(bal) = b.balances.last_mut() else {
                return;
            };
            match rest {
                ["Tp", "CdOrPrtry", "Cd"] => bal.code = Some(v.to_string()),
                ["Amt"] => bal.amount = Some(at()),
                ["CdtDbtInd"] => bal.debit = v == "DBIT",
                ["Dt", "Dt" | "DtTm"] => bal.date = Some(v.to_string()),
                _ => {}
            }
        }
        ["TxsSummry", rest @ ..] => {
            let s = &mut b.summary;
            match rest {
                ["TtlNtries", "NbOfNtries"] => s.nb = Some(at()),
                ["TtlNtries", "Sum"] => s.sum = Some(at()),
                ["TtlNtries", "TtlNetNtry", "Amt"] => s.net_amount = Some(at()),
                ["TtlNtries", "TtlNetNtry", "CdtDbtInd"] => s.net_debit = v == "DBIT",
                ["TtlCdtNtries", "NbOfNtries"] => s.credit_nb = Some(at()),
                ["TtlCdtNtries", "Sum"] => s.credit_sum = Some(at()),
                ["TtlDbtNtries", "NbOfNtries"] => s.debit_nb = Some(at()),
                ["TtlDbtNtries", "Sum"] => s.debit_sum = Some(at()),
                _ => {}
            }
        }
        ["Ntry", rest @ ..] => {
            let Some(n) = b.entries.last_mut() else {
                return;
            };
            match rest {
                ["Amt"] => n.amount = Some(v.to_string()),
                ["CdtDbtInd"] => n.debit = Some(v == "DBIT"),
                ["Sts", "Cd"] | ["Sts"] => n.status = Some(v.to_string()),
                ["BookgDt", "Dt" | "DtTm"] => own(&mut n.booking_date),
                ["AddtlNtryInf"] => {
                    own(&mut n.additional_info);
                    n.detail("Posting text", v);
                }
                ["NtryRef"] => n.detail("Entry reference", v),
                ["AcctSvcrRef"] => n.detail("Bank reference", v),
                ["ValDt", "Dt" | "DtTm"] => n.detail("Value date", v),
                ["RvslInd"] if v == "true" => n.detail("Reversal", "yes"),
                ["NtryDtls", "Btch", "NbOfTxs"] => n.detail("Batch: transactions", v),
                ["NtryDtls", "Btch", "PmtInfId"] => n.detail("Batch: PmtInfId", v),
                ["NtryDtls", "Btch", "TtlAmt"] => n.detail_amount("Batch: total", v),
                ["BkTxCd", code @ ..] | ["NtryDtls", "TxDtls", "BkTxCd", code @ ..] => match code {
                    ["Domn", "Cd"] => own(&mut n.domain),
                    ["Domn", "Fmly", "Cd"] => own(&mut n.family),
                    ["Domn", "Fmly", "SubFmlyCd"] => own(&mut n.sub_family),
                    ["Prtry", "Cd"] => own(&mut n.proprietary),
                    _ => {}
                },
                ["NtryDtls", "TxDtls", tx @ ..] => {
                    if n.tx_count <= 1 {
                        tx_detail(n, tx, v);
                    }
                    match tx {
                        ["Refs", "EndToEndId"] => own(&mut n.end_to_end_id),
                        ["RmtInf", "Ustrd"] => n.ustrd.push(v.to_string()),
                        ["RltdPties", "Dbtr", "Pty", "Nm"] | ["RltdPties", "Dbtr", "Nm"] => {
                            own(&mut n.debtor_name)
                        }
                        ["RltdPties", "DbtrAcct", "Id", "IBAN"] => own(&mut n.debtor_iban),
                        ["RltdPties", "Cdtr", "Pty", "Nm"] | ["RltdPties", "Cdtr", "Nm"] => {
                            own(&mut n.creditor_name)
                        }
                        ["RltdPties", "CdtrAcct", "Id", "IBAN"] => own(&mut n.creditor_iban),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

/// Totals of a block's entries.
#[derive(Default)]
struct Totals {
    count: u64,
    sum: Amount,
    credit_count: u64,
    credit_sum: Amount,
    debit_count: u64,
    debit_sum: Amount,
    booked_net: Amount,
    /// False if an amount or its credit/debit indicator is missing or malformed.
    known: bool,
}

fn totals(b: &Block) -> Totals {
    let mut t = Totals {
        known: true,
        ..Totals::default()
    };
    for n in &b.entries {
        t.count += 1;
        let (Some(a), Some(debit)) = (n.amount.as_deref().and_then(Amount::parse), n.debit) else {
            t.known = false;
            continue;
        };
        t.sum.0 += a.0;
        if debit {
            t.debit_count += 1;
            t.debit_sum.0 += a.0;
        } else {
            t.credit_count += 1;
            t.credit_sum.0 += a.0;
        }
        if n.booked() {
            t.booked_net.0 += if debit { -a.0 } else { a.0 };
        }
    }
    t
}

const SUMMARY_HINT: &str =
    "The transaction summary should describe the entries of this statement; check whether entries are missing.";

/// Plausibility warnings for camt.052/053/054 (none for other documents).
pub fn check(xml: &str) -> Vec<Message> {
    let mut out = Vec::new();
    for b in parse(xml) {
        check_block(&b, &mut out);
    }
    out
}

fn check_block(b: &Block, out: &mut Vec<Message>) {
    let t = totals(b);
    let word = block_word(&b.kind);
    let s = &b.summary;
    for (declared, actual, element, what) in [
        (&s.nb, t.count, "TtlNtries", ""),
        (&s.credit_nb, t.credit_count, "TtlCdtNtries", "credit "),
        (&s.debit_nb, t.debit_count, "TtlDbtNtries", "debit "),
    ] {
        let Some(d) = declared else { continue };
        if d.value.parse::<u64>().ok() != Some(actual) {
            let entries = if actual == 1 { "entry" } else { "entries" };
            out.push(warning(
                format!(
                    "{element}/NbOfNtries is {}, but this {word} has {actual} {what}{entries}.",
                    d.value
                ),
                d.line,
                SUMMARY_HINT,
            ));
        }
    }
    if !t.known {
        return; // an entry without a readable amount: sums can't be compared
    }
    for (declared, actual, element, what) in [
        (&s.sum, t.sum, "TtlNtries", "entry"),
        (&s.credit_sum, t.credit_sum, "TtlCdtNtries", "credit"),
        (&s.debit_sum, t.debit_sum, "TtlDbtNtries", "debit"),
    ] {
        let Some(d) = declared else { continue };
        if Amount::parse(&d.value).is_some_and(|v| v != actual) {
            out.push(warning(
                format!(
                    "{element}/Sum is {}, but the {what} amounts of this {word} add up to {}.",
                    d.value,
                    actual.display()
                ),
                d.line,
                SUMMARY_HINT,
            ));
        }
    }
    if let Some((d, v)) = s
        .net_amount
        .as_ref()
        .and_then(|d| Amount::parse(&d.value).map(|v| (d, v)))
    {
        let declared = if s.net_debit { Amount(-v.0) } else { v };
        let net = Amount(t.credit_sum.0 - t.debit_sum.0);
        if declared != net {
            out.push(warning(
                format!(
                    "TtlNetNtry is {} {}, but credits minus debits of this {word} are {}.",
                    d.value,
                    if s.net_debit { "DBIT" } else { "CRDT" },
                    net.display_signed()
                ),
                d.line,
                SUMMARY_HINT,
            ));
        }
    }
    check_balances(b, &t, out);
}

/// Opening booked balance plus booked entries must give the closing booked balance.
fn check_balances(b: &Block, t: &Totals, out: &mut Vec<Message>) {
    let find = |codes: &[&str]| {
        codes.iter().find_map(|c| {
            b.balances
                .iter()
                .find(|bal| bal.code.as_deref() == Some(*c))
        })
    };
    let (Some(open), Some(close)) = (find(&["OPBD", "PRCD"]), find(&["CLBD"])) else {
        return;
    };
    let (Some(o), Some(c), Some(at)) = (open.signed(), close.signed(), close.amount.as_ref())
    else {
        return;
    };
    // Sums across currencies mean nothing; skip mixed files.
    let ccys: Vec<&str> = [open.currency.as_deref(), close.currency.as_deref()]
        .into_iter()
        .chain(b.entries.iter().map(|n| n.currency.as_deref()))
        .flatten()
        .collect();
    if ccys.windows(2).any(|w| w[0] != w[1]) {
        return;
    }
    let expected = Amount(o.0 + t.booked_net.0);
    if expected != c {
        out.push(warning(
            format!(
                "Closing balance CLBD is {}, but opening balance {} {} plus the booked entries ({}) gives {}.",
                c.display_signed(),
                open.code.as_deref().unwrap_or("OPBD"),
                o.display_signed(),
                t.booked_net.display_signed(),
                expected.display_signed()
            ),
            at.line,
            "A statement must add up: check whether entries are missing or the file was changed.",
        ));
    }
}

fn block_word(kind: &str) -> &'static str {
    match kind {
        "Rpt" => "report",
        "Ntfctn" => "notification",
        _ => "statement",
    }
}

/// One balance as shown in the Overview; `amount` is signed (`-` for DBIT).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceSummary {
    pub code: Option<String>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub date: Option<String>,
}

/// One statement/report/notification block for the Overview.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementSummary {
    /// "Stmt", "Rpt" or "Ntfctn".
    pub kind: String,
    pub id: Option<String>,
    pub electronic_seq_nb: Option<String>,
    pub legal_seq_nb: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub iban: Option<String>,
    pub currency: Option<String>,
    pub owner: Option<String>,
    pub bic: Option<String>,
    pub balances: Vec<BalanceSummary>,
    pub entry_count: u64,
    pub credit_count: u64,
    pub credit_sum: String,
    pub debit_count: u64,
    pub debit_sum: String,
}

/// Overview blocks and one Transactions row per entry (empty for non-camt files).
/// The row's counterparty is the debtor of a credit and the creditor of a debit.
pub fn summarize(xml: &str) -> (Vec<StatementSummary>, Vec<RemittanceEntry>) {
    let mut statements = Vec::new();
    let mut rows = Vec::new();
    for b in parse(xml) {
        let t = totals(&b);
        statements.push(StatementSummary {
            kind: b.kind.clone(),
            id: b.id.clone(),
            electronic_seq_nb: b.electronic_seq_nb.clone(),
            legal_seq_nb: b.legal_seq_nb.clone(),
            from: b.from.clone(),
            to: b.to.clone(),
            iban: b.iban.clone(),
            currency: b.currency.clone(),
            owner: b.owner.clone(),
            bic: b.bic.clone(),
            balances: b
                .balances
                .iter()
                .map(|bal| BalanceSummary {
                    code: bal.code.clone(),
                    amount: bal
                        .signed()
                        .map(Amount::display_signed)
                        .or_else(|| bal.amount.as_ref().map(|a| a.value.clone())),
                    currency: bal.currency.clone(),
                    date: bal.date.clone(),
                })
                .collect(),
            entry_count: t.count,
            credit_count: t.credit_count,
            credit_sum: t.credit_sum.display(),
            debit_count: t.debit_count,
            debit_sum: t.debit_sum.display(),
        });
        for mut n in b.entries {
            let (iso_tx_code, gvc) = (n.iso_code(), n.gvc());
            let mut details = std::mem::take(&mut n.details);
            details.sort_by_key(|(l, _)| {
                DETAIL_ORDER
                    .iter()
                    .position(|o| o == l)
                    .unwrap_or(usize::MAX)
            });
            let details = details
                .into_iter()
                .map(|(label, value)| Detail {
                    label: label.to_string(),
                    value,
                })
                .collect();
            let debit = n.debit == Some(true);
            let (name, iban) = if debit {
                (n.creditor_name, n.creditor_iban)
            } else {
                (n.debtor_name, n.debtor_iban)
            };
            let ustrd = if n.ustrd.is_empty() {
                n.additional_info
            } else {
                Some(n.ustrd.join("\n"))
            };
            rows.push(RemittanceEntry {
                end_to_end_id: n.end_to_end_id,
                ustrd,
                name,
                iban,
                amount: n.amount,
                currency: n.currency,
                iso_tx_code,
                gvc,
                prtry_tx_code: n.proprietary,
                details,
                booking_date: n.booking_date,
                credit_debit: n.debit.map(|d| if d { "DBIT" } else { "CRDT" }.to_string()),
                ..RemittanceEntry::default()
            });
        }
    }
    (statements, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A formatted camt.053.001.08 statement, one element per line. `entries`
    /// are (amount, CdtDbtInd, status).
    fn camt053(
        open: (&str, &str),
        close: (&str, &str),
        summary: &str,
        entries: &[(&str, &str, &str)],
    ) -> String {
        let bal = |code: &str, (amt, ind): (&str, &str)| {
            format!(
                "      <Bal>\n        <Tp>\n          <CdOrPrtry>\n            <Cd>{code}</Cd>\n          </CdOrPrtry>\n        </Tp>\n        <Amt Ccy=\"EUR\">{amt}</Amt>\n        <CdtDbtInd>{ind}</CdtDbtInd>\n        <Dt>\n          <Dt>2026-11-02</Dt>\n        </Dt>\n      </Bal>\n"
            )
        };
        let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:camt.053.001.08\">\n  <BkToCstmrStmt>\n    <GrpHdr>\n      <MsgId>M1</MsgId>\n    </GrpHdr>\n    <Stmt>\n      <Id>S1</Id>\n      <ElctrncSeqNb>42</ElctrncSeqNb>\n      <Acct>\n        <Id>\n          <IBAN>DE89370400440532013000</IBAN>\n        </Id>\n        <Ccy>EUR</Ccy>\n      </Acct>\n");
        x += &bal("PRCD", open);
        x += &bal("CLBD", close);
        x += summary;
        for (i, (amt, ind, sts)) in entries.iter().enumerate() {
            let party = if *ind == "DBIT" { "Cdtr" } else { "Dbtr" };
            x += &format!(
                "      <Ntry>\n        <Amt Ccy=\"EUR\">{amt}</Amt>\n        <CdtDbtInd>{ind}</CdtDbtInd>\n        <Sts>\n          <Cd>{sts}</Cd>\n        </Sts>\n        <BookgDt>\n          <Dt>2026-11-02</Dt>\n        </BookgDt>\n        <NtryDtls>\n          <TxDtls>\n            <Refs>\n              <EndToEndId>E2E-{i}</EndToEndId>\n            </Refs>\n            <RltdPties>\n              <{party}>\n                <Pty>\n                  <Nm>Party {i}</Nm>\n                </Pty>\n              </{party}>\n              <{party}Acct>\n                <Id>\n                  <IBAN>DE89370400440532013000</IBAN>\n                </Id>\n              </{party}Acct>\n            </RltdPties>\n            <RmtInf>\n              <Ustrd>Invoice {i}</Ustrd>\n            </RmtInf>\n          </TxDtls>\n        </NtryDtls>\n      </Ntry>\n"
            );
        }
        x += "    </Stmt>\n  </BkToCstmrStmt>\n</Document>\n";
        x
    }

    fn summary(nb: &str, sum: &str, net: (&str, &str)) -> String {
        format!(
            "      <TxsSummry>\n        <TtlNtries>\n          <NbOfNtries>{nb}</NbOfNtries>\n          <Sum>{sum}</Sum>\n          <TtlNetNtry>\n            <Amt>{}</Amt>\n            <CdtDbtInd>{}</CdtDbtInd>\n          </TtlNetNtry>\n        </TtlNtries>\n      </TxsSummry>\n",
            net.0, net.1
        )
    }

    const TWO: &[(&str, &str, &str)] = &[("100.00", "CRDT", "BOOK"), ("30.50", "DBIT", "BOOK")];

    fn line_of(xml: &str, needle: &str) -> u32 {
        xml.lines().position(|l| l.contains(needle)).unwrap() as u32 + 1
    }

    #[test]
    fn a_consistent_statement_has_no_warnings() {
        let xml = camt053(
            ("1000.00", "CRDT"),
            ("1069.50", "CRDT"),
            &summary("2", "130.50", ("69.50", "CRDT")),
            TWO,
        );
        assert_eq!(check(&xml), vec![]);
    }

    #[test]
    fn cdata_text_is_read() {
        let xml = camt053(
            ("1000.00", "CRDT"),
            ("1069.50", "CRDT"),
            &summary("2", "130.50", ("69.50", "CRDT")),
            TWO,
        )
        .replace("<Nm>Party 0</Nm>", "<Nm><![CDATA[A & B <Test>]]></Nm>")
        .replace(
            "<Ustrd>Invoice 1</Ustrd>",
            "<Ustrd><![CDATA[Invoice <1> & more]]></Ustrd>",
        )
        .replace(
            "<Amt Ccy=\"EUR\">100.00</Amt>",
            "<Amt Ccy=\"EUR\"><![CDATA[100.00]]></Amt>",
        );
        // The CDATA amount counts: the balances add up, and a wrong CLBD is
        // still caught (an unread amount would skip the balance check).
        assert_eq!(check(&xml), vec![]);
        assert_eq!(check(&xml.replace(">1069.50<", ">1070.00<")).len(), 1);
        let (_, rows) = summarize(&xml);
        assert_eq!(rows[0].name.as_deref(), Some("A & B <Test>"));
        assert_eq!(rows[0].amount.as_deref(), Some("100.00"));
        assert_eq!(rows[1].ustrd.as_deref(), Some("Invoice <1> & more"));
    }

    #[test]
    fn a_closing_balance_that_does_not_add_up_is_reported() {
        let xml = camt053(("1000.00", "CRDT"), ("1070.00", "CRDT"), "", TWO);
        let w = check(&xml);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(
            w[0].text,
            "Closing balance CLBD is 1070.00, but opening balance PRCD 1000.00 plus the booked entries (69.50) gives 1069.50."
        );
        assert_eq!(w[0].line, Some(line_of(&xml, "1070.00")));
    }

    #[test]
    fn debit_balances_and_pending_entries_are_handled() {
        // -50.00 + 100.00 (booked) = 50.00; the pending debit does not count.
        let xml = camt053(
            ("50.00", "DBIT"),
            ("50.00", "CRDT"),
            "",
            &[("100.00", "CRDT", "BOOK"), ("20.00", "DBIT", "PDNG")],
        );
        assert_eq!(check(&xml), vec![]);
    }

    #[test]
    fn a_wrong_transaction_summary_is_reported() {
        let xml = camt053(
            ("1000.00", "CRDT"),
            ("1069.50", "CRDT"),
            &summary("3", "130.00", ("69.50", "DBIT")),
            TWO,
        );
        let texts: Vec<String> = check(&xml).into_iter().map(|m| m.text).collect();
        assert_eq!(
            texts,
            vec![
                "TtlNtries/NbOfNtries is 3, but this statement has 2 entries.",
                "TtlNtries/Sum is 130.00, but the entry amounts of this statement add up to 130.50.",
                "TtlNetNtry is 69.50 DBIT, but credits minus debits of this statement are 69.50.",
            ]
        );
    }

    #[test]
    fn other_documents_are_ignored() {
        let xml = "<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.001.001.09\">\n<CstmrCdtTrfInitn>\n<Stmt>\n<Ntry/>\n</Stmt>\n</CstmrCdtTrfInitn>\n</Document>\n";
        assert_eq!(check(xml), vec![]);
        assert!(summarize(xml).0.is_empty());
    }

    #[test]
    fn summary_lists_balances_and_entries_with_their_counterparty() {
        let xml = camt053(("1000.00", "CRDT"), ("1069.50", "CRDT"), "", TWO);
        let (stmts, rows) = summarize(&xml);
        assert_eq!(stmts.len(), 1);
        let s = &stmts[0];
        assert_eq!(s.kind, "Stmt");
        assert_eq!(s.id.as_deref(), Some("S1"));
        assert_eq!(s.electronic_seq_nb.as_deref(), Some("42"));
        assert_eq!(s.iban.as_deref(), Some("DE89370400440532013000"));
        assert_eq!(s.balances.len(), 2);
        assert_eq!(s.balances[1].code.as_deref(), Some("CLBD"));
        assert_eq!(s.balances[1].amount.as_deref(), Some("1069.50"));
        assert_eq!((s.entry_count, s.credit_count, s.debit_count), (2, 1, 1));
        assert_eq!(
            (s.credit_sum.as_str(), s.debit_sum.as_str()),
            ("100.00", "30.50")
        );

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name.as_deref(), Some("Party 0"));
        assert_eq!(rows[0].credit_debit.as_deref(), Some("CRDT"));
        assert_eq!(rows[1].credit_debit.as_deref(), Some("DBIT"));
        assert_eq!(rows[1].ustrd.as_deref(), Some("Invoice 1"));
        assert_eq!(rows[1].booking_date.as_deref(), Some("2026-11-02"));
        assert_eq!(rows[1].end_to_end_id.as_deref(), Some("E2E-1"));
    }

    #[test]
    fn bank_transaction_codes_are_read_per_entry() {
        let codes = |prtry: &str| {
            format!("        <BkTxCd>\n          <Domn>\n            <Cd>PMNT</Cd>\n            <Fmly>\n              <Cd>RCDT</Cd>\n              <SubFmlyCd>ESCT</SubFmlyCd>\n            </Fmly>\n          </Domn>\n          <Prtry>\n            <Cd>{prtry}</Cd>\n            <Issr>DK</Issr>\n          </Prtry>\n        </BkTxCd>\n")
        };
        let xml = camt053(
            ("0.00", "CRDT"),
            ("0.00", "CRDT"),
            "",
            &[("1.00", "CRDT", "BOOK")],
        )
        .replace(
            "        <NtryDtls>",
            &format!("{}        <NtryDtls>", codes("166")),
        );
        let (_, rows) = summarize(&xml);
        assert_eq!(rows[0].iso_tx_code.as_deref(), Some("PMNT-RCDT-ESCT"));
        assert_eq!(rows[0].gvc.as_deref(), Some("166"));
        assert_eq!(rows[0].prtry_tx_code.as_deref(), Some("166"));
    }

    #[test]
    fn the_gvc_is_taken_from_swift_style_codes_and_transaction_details() {
        let gvc = |p: &str| {
            Entry {
                proprietary: Some(p.to_string()),
                ..Entry::default()
            }
            .gvc()
        };
        assert_eq!(gvc("NTRF+166"), Some("166".into()));
        assert_eq!(gvc("NRTI+159++901"), Some("159".into()));
        assert_eq!(gvc("201"), Some("201".into()));
        assert_eq!(gvc("NMSC"), None);
        assert_eq!(gvc("12"), None);

        // Only TxDtls carries a code: it is used for the entry.
        let xml = camt053(("0.00", "CRDT"), ("0.00", "CRDT"), "", &[("1.00", "CRDT", "BOOK")])
            .replace(
                "          <TxDtls>\n",
                "          <TxDtls>\n            <BkTxCd>\n              <Prtry>\n                <Cd>NTRF+191</Cd>\n              </Prtry>\n            </BkTxCd>\n",
            );
        let (_, rows) = summarize(&xml);
        assert_eq!(rows[0].gvc.as_deref(), Some("191"));
        assert_eq!(rows[0].iso_tx_code, None);
    }

    #[test]
    fn entry_details_are_collected_in_display_order() {
        let extra = "        <AcctSvcrRef>BANKREF-1</AcctSvcrRef>\n        <AddtlNtryInf>SEPA-Gutschrift</AddtlNtryInf>\n";
        let tx = "            <Refs>\n              <MndtId>M-77</MndtId>\n            </Refs>\n            <AmtDtls>\n              <InstdAmt>\n                <Amt Ccy=\"USD\">12.00</Amt>\n              </InstdAmt>\n            </AmtDtls>\n            <RtrInf>\n              <Rsn>\n                <Cd>AC04</Cd>\n              </Rsn>\n            </RtrInf>\n";
        let xml = camt053(
            ("0.00", "CRDT"),
            ("0.00", "CRDT"),
            "",
            &[("1.00", "CRDT", "BOOK")],
        )
        .replace("        <NtryDtls>", &format!("{extra}        <NtryDtls>"))
        .replace("          <TxDtls>\n", &format!("          <TxDtls>\n{tx}"));
        let (_, rows) = summarize(&xml);
        let d: Vec<(String, String)> = rows[0]
            .details
            .iter()
            .map(|d| (d.label.clone(), d.value.clone()))
            .collect();
        let get = |l: &str| d.iter().find(|(k, _)| k == l).map(|(_, v)| v.as_str());
        assert_eq!(d[0].0, "Posting text");
        assert_eq!(get("Posting text"), Some("SEPA-Gutschrift"));
        assert_eq!(get("Bank reference"), Some("BANKREF-1"));
        assert_eq!(get("Mandate ID"), Some("M-77"));
        assert_eq!(get("Instructed amount"), Some("12.00 USD"));
        assert_eq!(get("Return reason"), Some("AC04"));
        assert_eq!(get("Debtor"), Some("Party 0"));
        assert_eq!(get("EndToEndId"), Some("E2E-0"));
    }

    #[test]
    fn batch_entries_take_details_from_the_first_transaction_only() {
        let xml = camt053(("0.00", "CRDT"), ("0.00", "CRDT"), "", &[("1.00", "CRDT", "BOOK")])
            .replace(
                "        </NtryDtls>",
                "          <TxDtls>\n            <Refs>\n              <EndToEndId>SECOND</EndToEndId>\n            </Refs>\n          </TxDtls>\n        </NtryDtls>",
            );
        let (_, rows) = summarize(&xml);
        let e2e: Vec<&str> = rows[0]
            .details
            .iter()
            .filter(|d| d.label == "EndToEndId")
            .map(|d| d.value.as_str())
            .collect();
        assert_eq!(e2e, vec!["E2E-0"]);
    }

    #[test]
    fn debit_balances_are_shown_negative() {
        let xml = camt053(("50.00", "DBIT"), ("50.00", "DBIT"), "", &[]);
        let (stmts, _) = summarize(&xml);
        assert_eq!(stmts[0].balances[0].amount.as_deref(), Some("-50.00"));
    }
}
