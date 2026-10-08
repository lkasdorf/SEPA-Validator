//! Plausibility checks the XSD can't express, reported as warnings with the
//! line in the formatted text: control values (NbOfTxs/CtrlSum) against the
//! actual transactions, IBAN check digits, past execution dates and duplicate
//! EndToEndIds.

use std::collections::HashMap;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::model::{Message, Severity};

/// Run all checks on a (formatted) document. `today` is `YYYY-MM-DD`.
pub fn check(xml: &str, today: &str) -> Vec<Message> {
    let mut c = Checker {
        today,
        ..Checker::default()
    };
    let mut reader = Reader::from_str(xml);
    let mut lines = LineCounter::new(xml);
    let mut stack: Vec<String> = Vec::new();
    loop {
        let line = lines.at(reader.buffer_position() as usize);
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                c.start(&name, &stack, line);
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                c.start(&name, &stack, line);
            }
            Ok(Event::End(_)) => {
                if let Some(name) = stack.pop() {
                    c.end(&name);
                }
            }
            Ok(Event::Text(t)) => {
                if let Ok(v) = t.unescape() {
                    c.text(&stack, &v, line);
                }
            }
            Ok(Event::CData(t)) => c.text(&stack, &String::from_utf8_lossy(&t), line),
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    c.finish()
}

/// Today's date (UTC) as `YYYY-MM-DD`.
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Days since 1970-01-01 to a calendar date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// IBAN check digits (ISO 13616, mod 97).
pub fn iban_is_valid(iban: &str) -> bool {
    let iban = iban.trim();
    if iban.len() < 5 || !iban.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let rearranged = iban[4..].chars().chain(iban[..4].chars());
    let mut rem = 0u32;
    for c in rearranged {
        let v = c.to_digit(36).unwrap_or(0);
        rem = if v >= 10 {
            (rem * 100 + v) % 97
        } else {
            (rem * 10 + v) % 97
        };
    }
    rem == 1
}

/// Fixed-point amount with 5 decimals, the most ISO 20022 amounts allow.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Amount(i128);

impl Amount {
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let (int, frac) = s.split_once('.').unwrap_or((s, ""));
        let digits = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
        if int.is_empty() || int.len() > 25 || frac.len() > 5 || !digits(int) || !digits(frac) {
            return None;
        }
        format!("{int}{frac:0<5}").parse().ok().map(Amount)
    }

    /// At least two decimals, trailing zeros beyond that dropped (30.50, 0.125).
    fn display(self) -> String {
        let s = format!("{}.{:05}", self.0 / 100_000, self.0 % 100_000);
        let keep = s.len() - 3; // "x.yy": always keep two decimals
        let significant = s.trim_end_matches('0').len();
        s[..significant.max(keep)].to_string()
    }
}

/// Declared control values and the actual transactions of the file or a PmtInf.
#[derive(Default)]
struct Totals {
    nb_of_txs: Option<(String, u32)>,
    ctrl_sum: Option<(String, u32)>,
    count: u64,
    sum: Amount,
    sum_known: bool,
}

impl Totals {
    fn new() -> Self {
        Totals {
            sum_known: true,
            ..Totals::default()
        }
    }

    fn add(&mut self, amount: Option<Amount>) {
        self.count += 1;
        match amount {
            Some(a) => self.sum = Amount(self.sum.0 + a.0),
            None => self.sum_known = false,
        }
    }

    fn compare(&self, scope: &str, out: &mut Vec<Message>) {
        const HINT: &str =
            "Banks usually reject a file whose control values don't match its transactions.";
        let (contains, add_up) = if scope == "GrpHdr" {
            ("the file contains", "the transaction amounts add up to")
        } else {
            (
                "this payment block contains",
                "the amounts in this payment block add up to",
            )
        };
        if let Some((declared, line)) = &self.nb_of_txs {
            if declared.trim().parse::<u64>().ok() != Some(self.count) {
                let txs = if self.count == 1 {
                    "transaction"
                } else {
                    "transactions"
                };
                out.push(warning(
                    format!(
                        "{scope}/NbOfTxs is {declared}, but {contains} {} {txs}.",
                        self.count
                    ),
                    *line,
                    HINT,
                ));
            }
        }
        if let Some((declared, line)) = &self.ctrl_sum {
            if self.sum_known && Amount::parse(declared).is_some_and(|d| d != self.sum) {
                out.push(warning(
                    format!(
                        "{scope}/CtrlSum is {declared}, but {add_up} {}.",
                        self.sum.display()
                    ),
                    *line,
                    HINT,
                ));
            }
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Kind {
    #[default]
    Other,
    CreditTransfer,
    DirectDebit,
}

#[derive(Default)]
struct Checker<'a> {
    today: &'a str,
    kind: Kind,
    file: Option<Totals>,
    block: Option<Totals>,
    pending_amount: Option<Option<Amount>>,
    end_to_end: HashMap<String, u32>,
    /// First past-date warning per kind (index into `out`) and how many more followed,
    /// so a file with thousands of payment blocks gets one warning, not thousands.
    past_dates: HashMap<&'static str, (usize, u32)>,
    out: Vec<Message>,
}

impl Checker<'_> {
    fn tx_element(&self) -> &'static str {
        match self.kind {
            Kind::CreditTransfer => "CdtTrfTxInf",
            Kind::DirectDebit => "DrctDbtTxInf",
            Kind::Other => "",
        }
    }

    fn start(&mut self, name: &str, stack: &[String], _line: u32) {
        if stack.len() == 1 {
            self.kind = match name {
                "CstmrCdtTrfInitn" => Kind::CreditTransfer,
                "CstmrDrctDbtInitn" => Kind::DirectDebit,
                _ => Kind::Other,
            };
            if self.kind != Kind::Other {
                self.file = Some(Totals::new());
            }
        }
        if self.kind == Kind::Other {
            return;
        }
        if name == "PmtInf" {
            self.block = Some(Totals::new());
        } else if name == self.tx_element() {
            self.pending_amount = Some(None);
        }
    }

    fn end(&mut self, name: &str) {
        if self.kind == Kind::Other {
            return;
        }
        if name == self.tx_element() {
            let amount = self.pending_amount.take().flatten();
            for totals in [&mut self.file, &mut self.block].into_iter().flatten() {
                totals.add(amount);
            }
        } else if name == "PmtInf" {
            if let Some(block) = self.block.take() {
                block.compare("PmtInf", &mut self.out);
            }
        }
    }

    fn text(&mut self, stack: &[String], value: &str, line: u32) {
        let [.., parent, element] = stack else { return };
        let (parent, element) = (parent.as_str(), element.as_str());
        if element == "IBAN" && !iban_is_valid(value) {
            self.out.push(warning(
                "IBAN check digits are wrong (mod-97 check failed); probably a typo.".into(),
                line,
                "Compare the IBAN with the account holder's details.",
            ));
        }
        if self.kind == Kind::Other {
            return;
        }
        match (parent, element) {
            ("GrpHdr", "NbOfTxs") => {
                set(&mut self.file, |t| t.nb_of_txs = Some((value.into(), line)))
            }
            ("GrpHdr", "CtrlSum") => {
                set(&mut self.file, |t| t.ctrl_sum = Some((value.into(), line)))
            }
            ("PmtInf", "NbOfTxs") => set(&mut self.block, |t| {
                t.nb_of_txs = Some((value.into(), line))
            }),
            ("PmtInf", "CtrlSum") => {
                set(&mut self.block, |t| t.ctrl_sum = Some((value.into(), line)))
            }
            (_, "InstdAmt") if self.pending_amount.is_some() => {
                self.pending_amount = Some(Amount::parse(value));
            }
            ("PmtId", "EndToEndId") => self.end_to_end_id(value.trim(), line),
            ("PmtInf", "ReqdExctnDt") | ("ReqdExctnDt", "Dt" | "DtTm") => {
                self.date("execution", value, line);
            }
            ("PmtInf", "ReqdColltnDt") => self.date("collection", value, line),
            _ => {}
        }
    }

    fn end_to_end_id(&mut self, id: &str, line: u32) {
        if id.is_empty() || id == "NOTPROVIDED" {
            return;
        }
        if let Some(first) = self.end_to_end.get(id) {
            let text = format!("EndToEndId '{id}' is used more than once (first on line {first}).");
            self.out.push(warning(
                text,
                line,
                "EndToEndIds should be unique so each payment can be traced.",
            ));
        } else {
            self.end_to_end.insert(id.to_string(), line);
        }
    }

    fn date(&mut self, what: &'static str, value: &str, line: u32) {
        let date = value.trim().get(..10).unwrap_or("");
        let well_formed = date.len() == 10
            && date.bytes().enumerate().all(|(i, b)| {
                if i == 4 || i == 7 {
                    b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            });
        // 1999-01-01 is the German DK convention for "execute as soon as possible".
        if well_formed && date != "1999-01-01" && date < self.today {
            if let Some((_, more)) = self.past_dates.get_mut(what) {
                *more += 1;
                return;
            }
            self.past_dates.insert(what, (self.out.len(), 0));
            self.out.push(warning(
                format!("Requested {what} date {date} is in the past."),
                line,
                "Banks reject past dates or move them to the next business day; check before uploading.",
            ));
        }
    }

    fn finish(mut self) -> Vec<Message> {
        for (index, more) in self.past_dates.values() {
            if *more > 0 {
                let m = &mut self.out[*index];
                let blocks = if *more == 1 { "block" } else { "blocks" };
                m.text = m.text.replace(
                    " is in the past.",
                    &format!(" is in the past ({more} more payment {blocks} too)."),
                );
            }
        }
        if let Some(file) = self.file.take() {
            file.compare("GrpHdr", &mut self.out);
        }
        self.out.sort_by_key(|m| m.line);
        self.out
    }
}

fn set(totals: &mut Option<Totals>, f: impl FnOnce(&mut Totals)) {
    if let Some(t) = totals.as_mut() {
        f(t);
    }
}

fn warning(text: String, line: u32, hint: &str) -> Message {
    Message {
        severity: Severity::Warning,
        text,
        line: Some(line),
        column: None,
        hint: Some(hint.to_string()),
    }
}

/// Maps byte offsets to 1-based line numbers for offsets visited in order.
struct LineCounter<'a> {
    bytes: &'a [u8],
    pos: usize,
    line: u32,
}

impl<'a> LineCounter<'a> {
    fn new(text: &'a str) -> Self {
        LineCounter {
            bytes: text.as_bytes(),
            pos: 0,
            line: 1,
        }
    }

    fn at(&mut self, pos: usize) -> u32 {
        let pos = pos.min(self.bytes.len());
        if pos > self.pos {
            self.line += self.bytes[self.pos..pos]
                .iter()
                .filter(|&&b| b == b'\n')
                .count() as u32;
            self.pos = pos;
        }
        self.line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Severity;

    const TODAY: &str = "2026-10-09";

    /// One credit transfer: (EndToEndId, amount, creditor IBAN).
    type Tx<'a> = (&'a str, &'a str, &'a str);

    /// A formatted pain.001.001.09 file, one element per line (like the viewer).
    fn pain001(grp: (&str, &str), pmt: (&str, &str), date: &str, txs: &[Tx]) -> String {
        let mut x = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.001.001.09\">\n  <CstmrCdtTrfInitn>\n    <GrpHdr>\n      <MsgId>M1</MsgId>\n",
        );
        x += &format!(
            "      <NbOfTxs>{}</NbOfTxs>\n      <CtrlSum>{}</CtrlSum>\n    </GrpHdr>\n",
            grp.0, grp.1
        );
        x += "    <PmtInf>\n      <PmtInfId>P1</PmtInfId>\n";
        x += &format!(
            "      <NbOfTxs>{}</NbOfTxs>\n      <CtrlSum>{}</CtrlSum>\n",
            pmt.0, pmt.1
        );
        x += &format!("      <ReqdExctnDt>\n        <Dt>{date}</Dt>\n      </ReqdExctnDt>\n");
        x += "      <DbtrAcct>\n        <Id>\n          <IBAN>DE89370400440532013000</IBAN>\n        </Id>\n      </DbtrAcct>\n";
        for (e2e, amt, iban) in txs {
            x += &format!(
                "      <CdtTrfTxInf>\n        <PmtId>\n          <EndToEndId>{e2e}</EndToEndId>\n        </PmtId>\n        <Amt>\n          <InstdAmt Ccy=\"EUR\">{amt}</InstdAmt>\n        </Amt>\n        <CdtrAcct>\n          <Id>\n            <IBAN>{iban}</IBAN>\n          </Id>\n        </CdtrAcct>\n      </CdtTrfTxInf>\n"
            );
        }
        x += "    </PmtInf>\n  </CstmrCdtTrfInitn>\n</Document>\n";
        x
    }

    const OK_IBAN: &str = "DE89370400440532013000";
    const TWO_TXS: &[Tx] = &[("E2E-1", "10.25", OK_IBAN), ("E2E-2", "20.25", OK_IBAN)];

    fn line_of(xml: &str, needle: &str) -> u32 {
        xml.lines().position(|l| l.contains(needle)).unwrap() as u32 + 1
    }

    #[test]
    fn a_consistent_file_has_no_warnings() {
        let xml = pain001(("2", "30.50"), ("2", "30.50"), "2026-10-12", TWO_TXS);
        assert_eq!(check(&xml, TODAY), vec![]);
    }

    #[test]
    fn control_sums_are_compared_as_decimals() {
        let xml = pain001(
            ("2", "30.5"),
            ("2", "30.500"),
            "2026-10-12",
            &[("A", "10.5", OK_IBAN), ("B", "20", OK_IBAN)],
        );
        assert_eq!(check(&xml, TODAY), vec![]);
    }

    #[test]
    fn wrong_transaction_counts_are_reported_at_their_lines() {
        let xml = pain001(("3", "30.50"), ("1", "30.50"), "2026-10-12", TWO_TXS);
        let w = check(&xml, TODAY);
        assert_eq!(w.len(), 2, "{w:?}");
        assert_eq!(
            w[0].text,
            "GrpHdr/NbOfTxs is 3, but the file contains 2 transactions."
        );
        assert_eq!(w[0].line, Some(line_of(&xml, "<NbOfTxs>3")));
        assert_eq!(
            w[1].text,
            "PmtInf/NbOfTxs is 1, but this payment block contains 2 transactions."
        );
        assert!(w.iter().all(|m| m.severity == Severity::Warning));
    }

    #[test]
    fn wrong_control_sums_are_reported() {
        let xml = pain001(("2", "31.00"), ("2", "30.00"), "2026-10-12", TWO_TXS);
        let texts: Vec<String> = check(&xml, TODAY).into_iter().map(|m| m.text).collect();
        assert_eq!(
            texts,
            vec![
                "GrpHdr/CtrlSum is 31.00, but the transaction amounts add up to 30.50.",
                "PmtInf/CtrlSum is 30.00, but the amounts in this payment block add up to 30.50.",
            ]
        );
    }

    #[test]
    fn a_wrong_iban_check_digit_is_reported() {
        let xml = pain001(
            ("2", "30.50"),
            ("2", "30.50"),
            "2026-10-12",
            &[
                ("A", "10.25", OK_IBAN),
                ("B", "20.25", "DE89370400440532013001"),
            ],
        );
        let w = check(&xml, TODAY);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].text.contains("check digits"), "{}", w[0].text);
        assert_eq!(w[0].line, Some(line_of(&xml, "532013001")));
    }

    #[test]
    fn a_past_execution_date_is_reported_but_today_and_asap_are_not() {
        let past = pain001(("2", "30.50"), ("2", "30.50"), "2026-10-01", TWO_TXS);
        let w = check(&past, TODAY);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(
            w[0].text,
            "Requested execution date 2026-10-01 is in the past."
        );
        for date in [TODAY, "1999-01-01"] {
            let xml = pain001(("2", "30.50"), ("2", "30.50"), date, TWO_TXS);
            assert_eq!(check(&xml, TODAY), vec![], "date {date}");
        }
    }

    #[test]
    fn a_duplicate_end_to_end_id_is_reported_at_the_repeat() {
        let xml = pain001(
            ("2", "30.50"),
            ("2", "30.50"),
            "2026-10-12",
            &[("E2E-1", "10.25", OK_IBAN), ("E2E-1", "20.25", OK_IBAN)],
        );
        let w = check(&xml, TODAY);
        assert_eq!(w.len(), 1, "{w:?}");
        let first = xml.lines().position(|l| l.contains("E2E-1")).unwrap() as u32 + 1;
        assert_eq!(
            w[0].text,
            format!("EndToEndId 'E2E-1' is used more than once (first on line {first}).")
        );
        assert!(w[0].line.unwrap() > first);
    }

    #[test]
    fn notprovided_end_to_end_ids_may_repeat() {
        let xml = pain001(
            ("2", "30.50"),
            ("2", "30.50"),
            "2026-10-12",
            &[
                ("NOTPROVIDED", "10.25", OK_IBAN),
                ("NOTPROVIDED", "20.25", OK_IBAN),
            ],
        );
        assert_eq!(check(&xml, TODAY), vec![]);
    }

    #[test]
    fn direct_debits_use_the_collection_date_and_their_own_amounts() {
        let xml = "<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.008.001.02\">\n<CstmrDrctDbtInitn>\n<GrpHdr>\n<NbOfTxs>1</NbOfTxs>\n<CtrlSum>5.00</CtrlSum>\n</GrpHdr>\n<PmtInf>\n<ReqdColltnDt>2026-10-02</ReqdColltnDt>\n<DrctDbtTxInf>\n<PmtId>\n<EndToEndId>D1</EndToEndId>\n</PmtId>\n<InstdAmt Ccy=\"EUR\">5.00</InstdAmt>\n</DrctDbtTxInf>\n</PmtInf>\n</CstmrDrctDbtInitn>\n</Document>\n";
        let texts: Vec<String> = check(xml, TODAY).into_iter().map(|m| m.text).collect();
        assert_eq!(
            texts,
            vec!["Requested collection date 2026-10-02 is in the past."]
        );
    }

    #[test]
    fn past_dates_in_many_payment_blocks_are_reported_once() {
        let block = |d: &str| {
            format!(
                "<PmtInf>
<ReqdColltnDt>{d}</ReqdColltnDt>
</PmtInf>
"
            )
        };
        let xml = format!(
            "<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.008.001.02\">
<CstmrDrctDbtInitn>
{}{}{}</CstmrDrctDbtInitn>
</Document>
",
            block("2026-10-01"),
            block("2026-10-02"),
            block("2026-10-03"),
        );
        let w = check(&xml, TODAY);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(
            w[0].text,
            "Requested collection date 2026-10-01 is in the past (2 more payment blocks too)."
        );
    }

    #[test]
    fn other_message_types_only_get_the_iban_check() {
        let xml = "<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:camt.054.001.08\">\n<BkToCstmrDbtCdtNtfctn>\n<GrpHdr>\n<NbOfTxs>9</NbOfTxs>\n</GrpHdr>\n<Ntfctn>\n<Acct>\n<Id>\n<IBAN>DE89370400440532013001</IBAN>\n</Id>\n</Acct>\n</Ntfctn>\n</BkToCstmrDbtCdtNtfctn>\n</Document>\n";
        let w = check(xml, TODAY);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].text.contains("check digits"));
    }

    #[test]
    fn calendar_dates_from_days_since_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_358), (2023, 1, 1));
        assert_eq!(civil_from_days(20_735), (2026, 10, 9));
        assert_eq!(today().len(), 10);
    }

    #[test]
    fn amounts_display_with_at_least_two_decimals() {
        let show = |s: &str| Amount::parse(s).unwrap().display();
        assert_eq!(show("30.5"), "30.50");
        assert_eq!(show("7"), "7.00");
        assert_eq!(show("0.125"), "0.125");
        assert!(Amount::parse("1,5").is_none());
    }

    #[test]
    fn iban_check_digits() {
        assert!(iban_is_valid("DE89370400440532013000"));
        assert!(iban_is_valid("CH9300762011623852957"));
        assert!(!iban_is_valid("DE89370400440532013001"));
        assert!(!iban_is_valid("XX"));
    }
}
