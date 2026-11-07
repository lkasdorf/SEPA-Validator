//! Synthetic camt.053.001.08 bank statements from a pain.001/pain.008 file,
//! as if the bank had booked it: one statement per account and booking day,
//! days chained (each closing balance opens the next day). Everything the bank
//! would add (IDs, references, the bank itself) is marked synthetic; data from
//! the source file (IBANs, names, references, remittance text) is copied.

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
    } = t;
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
