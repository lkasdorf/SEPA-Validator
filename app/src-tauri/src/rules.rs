//! Usage rules of the German banking industry (DK, "DFÜ-Abkommen Anlage 3"
//! V3.9, column "EPC-/DK-Regeln") that the XSD can't express. A file that
//! breaks them is schema-valid, but banks may reject it, so they are errors.
//! Only wrong RF check digits in a creditor reference are a warning: banks
//! needn't check them.
//!
//! The rules cover SEPA credit transfers and direct debits (pain.001,
//! pain.008). Swiss files and the DK urgent/foreign credit transfer subsets
//! have rules of their own and are skipped.

use std::collections::HashSet;

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use serde::{Deserialize, Serialize};

use crate::model::{Message, Severity};
use crate::plausibility::{iban_is_valid, LineCounter};
use crate::schema::Variant;
use crate::validator;

const PAIN001: &str = "urn:iso:std:iso:20022:tech:xsd:pain.001.";
const PAIN008: &str = "urn:iso:std:iso:20022:tech:xsd:pain.008.";

/// References whose characters the DK schemas restrict further (no umlauts,
/// & * $ %) and that must not start or end with "/" or contain "//" (EPC230-15).
const IDS: &[&str] = &[
    "MsgId",
    "PmtInfId",
    "InstrId",
    "EndToEndId",
    "MndtId",
    "OrgnlMndtId",
];

/// EU and EEA countries, plus EU regions that have ISO codes of their own.
const EEA: &[&str] = &[
    "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "GR", "HR", "HU", "IE", "IT",
    "LT", "LU", "LV", "MT", "NL", "PL", "PT", "RO", "SE", "SI", "SK", // EU
    "IS", "LI", "NO", // EEA
    "AX", "GF", "GP", "MF", "MQ", "RE", "YT", // EU regions
];

const CHARS_HINT: &str = "Allowed are a-z, A-Z, 0-9, space and ' : ? , - ( + . ) /. \
    Banks also accept Ä Ö Ü ä ö ü ß & * $ %. They may replace other characters or reject the file.";
const ID_CHARS_HINT: &str =
    "References and IDs allow only a-z, A-Z, 0-9, space and ' : ? , - ( + . ) /.";

/// Parties whose names DK limits to 70 characters and whose postal addresses
/// need a town and country.
const PARTIES: &[&str] = &[
    "InitgPty",
    "Dbtr",
    "Cdtr",
    "UltmtDbtr",
    "UltmtCdtr",
    "OrgnlCdtrSchmeId",
];

/// Groups of DK rules that can be switched off in the app's settings.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Group {
    /// Character set (also in IDs) and namespace prefixes.
    Characters,
    /// Slashes at the start or end of references and IDs, or "//".
    References,
    /// Service level, local instrument, CORE/B2B and elements on one level only.
    PaymentType,
    /// Creditor identifier and mandate amendments.
    Creditor,
    /// Name lengths, amounts and the direct debit currency.
    AmountsNames,
    /// Postal addresses and remittance information.
    AddressesRemittance,
}

/// A finding with the rule group it belongs to.
type Findings = Vec<(Group, Message)>;

/// Check a (formatted) document against all DK usage rules.
#[cfg(test)]
pub fn check(xml: &str) -> Vec<Message> {
    check_without(xml, &[])
}

/// Check a (formatted) document against the DK usage rules, except those in
/// the `disabled` groups.
pub fn check_without(xml: &str, disabled: &[Group]) -> Vec<Message> {
    let Some((kind, iso2019)) = Kind::of(xml) else {
        return Vec::new();
    };
    let mut rules = Rules::new(kind, iso2019);
    let mut reader = Reader::from_str(xml);
    let mut lines = LineCounter::new(xml);
    let mut stack: Vec<String> = Vec::new();
    loop {
        let line = lines.at(reader.buffer_position() as usize);
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = rules.element(&e, &stack, line);
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = rules.element(&e, &stack, line);
                stack.push(name);
                rules.end(&stack);
                stack.pop();
            }
            Ok(Event::End(_)) => {
                rules.end(&stack);
                stack.pop();
            }
            Ok(Event::Text(t)) => {
                if let Ok(v) = t.unescape() {
                    rules.text(&stack, &v, line);
                }
            }
            Ok(Event::CData(t)) => rules.text(&stack, &String::from_utf8_lossy(&t), line),
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    rules
        .finish()
        .into_iter()
        .filter(|(group, _)| !disabled.contains(group))
        .map(|(_, message)| message)
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    CreditTransfer,
    DirectDebit,
}

impl Kind {
    /// The kind of SEPA file the rules apply to, or None for anything else, and
    /// whether it is an ISO 2019 version (pain.001.001.09, pain.008.001.08 or
    /// later), for which DK requires structured postal addresses.
    fn of(xml: &str) -> Option<(Self, bool)> {
        let bytes = xml.as_bytes();
        let ns = validator::namespace_in(bytes)?;
        let kind = if ns.starts_with(PAIN008) {
            Kind::DirectDebit
        } else if ns.starts_with(PAIN001) {
            Kind::CreditTransfer
        } else {
            return None;
        };
        if validator::is_swiss(bytes) {
            return None;
        }
        if kind == Kind::CreditTransfer
            && validator::dk_credit_transfer_kind(bytes) != Variant::Standard
        {
            return None;
        }
        let version: u32 = ns.rsplit('.').next()?.parse().ok()?;
        let iso2019 = match kind {
            Kind::CreditTransfer => version >= 9,
            Kind::DirectDebit => version >= 8,
        };
        Some((kind, iso2019))
    }
}

/// True if the innermost elements of `stack` are `tail`.
fn ends_with(stack: &[String], tail: &[&str]) -> bool {
    stack.len() >= tail.len()
        && stack[stack.len() - tail.len()..]
            .iter()
            .zip(tail)
            .all(|(a, b)| a == b)
}

fn error(out: &mut Findings, group: Group, line: u32, text: String, hint: &str) {
    out.push((
        group,
        Message {
            severity: Severity::Error,
            text,
            line: Some(line),
            column: None,
            hint: Some(hint.into()),
        },
    ));
}

struct Rules {
    kind: Kind,
    /// ISO 2019 versions: postal addresses need a town and country.
    iso2019: bool,
    out: Findings,
    /// Findings that may repeat throughout a file, reported once each.
    repeated: Vec<Repeated>,
    prefix_reported: bool,
    block: Block,
    tx: Option<Tx>,
    mandate: Option<Mandate>,
    amendment: Option<Amendment>,
    /// The first direct debit type (CORE/B2B) and its line, for the mixing rule.
    first_type: Option<(String, u32)>,
    mixing_reported: bool,
    /// Invalid creditor identifiers already reported (each once).
    bad_creditor_ids: HashSet<String>,
    remittance: Option<Remittance>,
    structured: Option<Structured>,
    address: Option<Address>,
}

impl Rules {
    fn new(kind: Kind, iso2019: bool) -> Self {
        Rules {
            kind,
            iso2019,
            out: Vec::new(),
            repeated: Vec::new(),
            prefix_reported: false,
            block: Block::default(),
            tx: None,
            mandate: None,
            amendment: None,
            first_type: None,
            mixing_reported: false,
            bad_creditor_ids: HashSet::new(),
            remittance: None,
            structured: None,
            address: None,
        }
    }

    /// The findings, with each repeated one once and a count of the others.
    fn finish(mut self) -> Findings {
        for r in self.repeated {
            let more = match r.more {
                0 => String::new(),
                n => format!("; {n} more in this file"),
            };
            let message = Message {
                severity: r.severity,
                text: format!("{} ({}{more}).", r.text, r.source),
                line: Some(r.line),
                column: None,
                hint: Some(r.hint.into()),
            };
            self.out.push((r.group, message));
        }
        self.out
    }

    /// A DK rule error that may repeat: reported at its first line only.
    fn repeat(
        &mut self,
        key: &'static str,
        group: Group,
        line: u32,
        text: impl FnOnce() -> String,
        hint: &'static str,
    ) {
        self.repeat_as(key, group, (Severity::Error, "DK rules"), line, text, hint);
    }

    fn repeat_as(
        &mut self,
        key: &'static str,
        group: Group,
        (severity, source): (Severity, &'static str),
        line: u32,
        text: impl FnOnce() -> String,
        hint: &'static str,
    ) {
        if let Some(r) = self.repeated.iter_mut().find(|r| r.key == key) {
            r.more += 1;
            return;
        }
        self.repeated.push(Repeated {
            key,
            group,
            severity,
            source,
            line,
            text: text(),
            hint,
            more: 0,
        });
    }

    /// An element starts; returns its local name.
    fn element(&mut self, e: &BytesStart, stack: &[String], line: u32) -> String {
        // DK Anlage 3, "Setzen von individuellen Präfixen".
        if !self.prefix_reported && e.name().prefix().is_some() {
            self.prefix_reported = true;
            error(
                &mut self.out,
                Group::Characters,
                line,
                format!(
                    "<{}> has a namespace prefix; DK files must not use prefixes (DK rules).",
                    String::from_utf8_lossy(e.name().as_ref())
                ),
                "Declare the namespace as xmlns=\"…\" on <Document> and write the elements \
                 without a prefix. Banks may reject files with prefixes.",
            );
        }
        let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
        if self.kind == Kind::DirectDebit && name == "InstdAmt" {
            let ccy = e
                .try_get_attribute("Ccy")
                .ok()
                .flatten()
                .and_then(|a| a.unescape_value().ok().map(|v| v.trim().to_string()));
            if let Some(ccy) = ccy.filter(|c| c != "EUR") {
                self.repeat(
                    "currency",
                    Group::AmountsNames,
                    line,
                    || format!("InstdAmt is in {ccy}; SEPA direct debits are in EUR"),
                    "Collect SEPA direct debits in EUR only.",
                );
            }
        }
        self.start(&name, stack, line);
        name
    }

    fn start(&mut self, name: &str, stack: &[String], line: u32) {
        if let Some(a) = self.amendment.as_mut() {
            a.start(name, stack, line);
        }
        let parent = stack.last().map(String::as_str);
        match (parent, name) {
            (_, "PmtInf") => self.block = Block::default(),
            (Some("PmtInf"), "PmtTpInf") => self.block.payment_type = true,
            (Some("PmtInf"), "CdtrSchmeId") => self.block.scheme_id = true,
            (_, "CdtTrfTxInf" | "DrctDbtTxInf") => {
                self.tx = Some(Tx {
                    line,
                    ..Tx::default()
                })
            }
            (_, "MndtRltdInf") => self.mandate = Some(Mandate::default()),
            (_, "AmdmntInfDtls") => {
                if let Some(m) = self.mandate.as_mut() {
                    m.details = true;
                }
                self.amendment = Some(Amendment::default());
            }
            (Some("PmtInf"), "UltmtDbtr" | "UltmtCdtr") => self.block.ultimate = true,
            (Some("PmtInf"), "ChrgBr") => self.block.charge_bearer = true,
            (_, "RmtInf") => self.remittance = Some(Remittance::default()),
            (Some("RmtInf"), "Ustrd") => {
                if let Some(r) = self.remittance.as_mut() {
                    r.unstructured.push(line);
                }
            }
            (Some("PstlAdr"), "TwnNm" | "Ctry" | "AdrLine") => {
                if let Some(a) = self.address.as_mut() {
                    match name {
                        "TwnNm" => a.town = true,
                        "Ctry" => a.country = true,
                        _ => a.lines += 1,
                    }
                }
            }
            (Some(owner), "PstlAdr") if self.iso2019 && PARTIES.contains(&owner) => {
                self.address = Some(Address {
                    owner: owner.to_string(),
                    line,
                    ..Address::default()
                })
            }
            _ => {}
        }
        // DK Anlage 3, 2.2.1.7: the structured remittance information holds
        // only the creditor reference, at most 140 characters with its tags.
        if let Some(s) = self.structured.as_mut() {
            s.length += 2 * name.len() + 5; // <name> and </name>
        }
        if self.structured.is_some() {
            if parent == Some("Strd") && name != "CdtrRefInf" {
                self.repeat(
                    "structured-branch",
                    Group::AddressesRemittance,
                    line,
                    || format!("Strd contains {name}; SEPA allows only CdtrRefInf there"),
                    "Structured remittance information may only hold the creditor reference \
                     (CdtrRefInf). Put other information in Ustrd instead.",
                );
            }
        } else if parent == Some("RmtInf") && name == "Strd" {
            self.structured = Some(Structured { line, length: 0 });
            if let Some(r) = self.remittance.as_mut() {
                r.structured.push(line);
            }
        }
        if let Some(tx) = self.tx.as_mut() {
            match (parent, name) {
                (Some("CdtTrfTxInf"), "UltmtDbtr") | (Some("DrctDbtTxInf"), "UltmtCdtr") => {
                    tx.ultimate = Some(line)
                }
                (Some("CdtTrfTxInf" | "DrctDbtTxInf"), "ChrgBr") => tx.charge_bearer = Some(line),
                (Some("CdtTrfTxInf" | "DrctDbtTxInf"), "PmtTpInf") => tx.payment_type = Some(line),
                (Some("DrctDbtTx"), "CdtrSchmeId") => tx.scheme_id = Some(line),
                (Some("DrctDbtTxInf"), "Dbtr") => tx.debtor = Some(line),
                (Some("Dbtr"), "PstlAdr") if ends_with(stack, &["DrctDbtTxInf", "Dbtr"]) => {
                    tx.debtor_address = true
                }
                _ => {}
            }
        }
    }

    /// The innermost element of `stack` ends.
    fn end(&mut self, stack: &[String]) {
        match stack.last().map(String::as_str) {
            Some("AmdmntInfDtls") => {
                if let Some(a) = self.amendment.take() {
                    a.finish(&mut self.out);
                }
            }
            Some("MndtRltdInf") => {
                if let Some(m) = self.mandate.take() {
                    m.finish(&mut self.out);
                }
            }
            Some("CdtTrfTxInf" | "DrctDbtTxInf") => {
                if let Some(tx) = self.tx.take() {
                    self.transaction(tx);
                }
            }
            Some("PmtInf") => {
                let ultimate = match self.kind {
                    Kind::CreditTransfer => "UltmtDbtr",
                    Kind::DirectDebit => "UltmtCdtr",
                };
                std::mem::take(&mut self.block).finish(&mut self.out, ultimate);
            }
            Some("RmtInf") => {
                if let Some(r) = self.remittance.take() {
                    self.remittance_finish(r);
                }
            }
            Some("Strd") => {
                if let Some(s) = self.structured.take() {
                    if s.length > 140 {
                        self.repeat(
                            "structured-length",
                            Group::AddressesRemittance,
                            s.line,
                            || {
                                format!(
                                    "Strd is {} characters long with its tags; SEPA allows at most 140",
                                    s.length
                                )
                            },
                            "Counted are the tags and content inside <Strd>, without <Strd> \
                             and </Strd> themselves.",
                        );
                    }
                }
            }
            Some("PstlAdr") => {
                if let Some(a) = self.address.take() {
                    self.address_finish(a);
                }
            }
            _ => {}
        }
    }

    /// DK Anlage 3, 2.2.1.7: one Ustrd or one Strd, not both.
    fn remittance_finish(&mut self, r: Remittance) {
        if let Some(&line) = r.unstructured.get(1) {
            self.repeat(
                "unstructured-twice",
                Group::AddressesRemittance,
                line,
                || "RmtInf has more than one Ustrd".into(),
                "SEPA allows one Ustrd of up to 140 characters per transaction.",
            );
        }
        if let Some(&line) = r.structured.get(1) {
            self.repeat(
                "structured-twice",
                Group::AddressesRemittance,
                line,
                || "RmtInf has more than one Strd".into(),
                "SEPA allows one structured creditor reference per transaction.",
            );
        }
        if let (Some(_), Some(&line)) = (r.unstructured.first(), r.structured.first()) {
            self.repeat(
                "unstructured-and-structured",
                Group::AddressesRemittance,
                line,
                || "RmtInf has both Ustrd and Strd; SEPA allows one or the other".into(),
                "Give either the remittance text (Ustrd) or a structured creditor reference \
                 (Strd), not both.",
            );
        }
    }

    /// DK Anlage 3 from ISO 2019 (GBIC_5): a postal address has a town and a
    /// country, and at most 2 address lines.
    fn address_finish(&mut self, a: Address) {
        let missing = match (a.town, a.country) {
            (true, true) => None,
            (false, true) => Some("TwnNm"),
            (true, false) => Some("Ctry"),
            (false, false) => Some("TwnNm and Ctry"),
        };
        if let Some(missing) = missing {
            let owner = &a.owner;
            self.repeat(
                "address-town-country",
                Group::AddressesRemittance,
                a.line,
                || format!("{owner}/PstlAdr has no {missing}"),
                "From pain.001.001.09 and pain.008.001.08 on (GBIC_5, since 5 Oct 2025) a postal \
                 address needs at least the town (TwnNm) and country (Ctry). Up to 2 AdrLine may \
                 add the rest, e.g. street and number.",
            );
        }
        if a.lines > 2 {
            let (owner, lines) = (&a.owner, a.lines);
            self.repeat(
                "address-lines",
                Group::AddressesRemittance,
                a.line,
                || format!("{owner}/PstlAdr has {lines} AdrLine; DK allows at most 2"),
                "Put street, building number and postcode in their own elements \
                 (StrtNm, BldgNb, PstCd) and keep at most 2 AdrLine.",
            );
        }
    }

    fn text(&mut self, stack: &[String], value: &str, line: u32) {
        if let Some(a) = self.amendment.as_mut() {
            a.text(stack, value);
        }
        let trimmed = value.trim();
        let Some(element) = stack.last().map(String::as_str) else {
            return;
        };
        if trimmed.is_empty() {
            return; // indentation between elements
        }
        if let Some(s) = self.structured.as_mut() {
            s.length += trimmed.chars().count();
        }
        self.content(stack, element, trimmed, line);
        if is_creditor_id(stack) {
            self.creditor_id(trimmed, line);
            return;
        }
        let is_id = IDS.contains(&element);
        self.characters(element, value, is_id, line);
        if is_id {
            self.slashes(element, trimmed, line);
        }
        if is_local_instrument(stack) {
            self.local_instrument(element, trimmed, line);
        }
        if self.kind == Kind::DirectDebit
            && ends_with(stack, &["PmtTpInf", "SvcLvl", "Cd"])
            && trimmed != "SEPA"
        {
            error(
                &mut self.out,
                Group::PaymentType,
                line,
                format!(
                    "SvcLvl '{trimmed}' is not allowed; a SEPA direct debit has SEPA (DK rules)."
                ),
                "Set PmtTpInf/SvcLvl/Cd to SEPA.",
            );
        }
        if ends_with(stack, &["MndtRltdInf", "AmdmntInd"]) && matches!(trimmed, "true" | "1") {
            if let Some(m) = self.mandate.as_mut() {
                m.amendment = Some(line);
            }
        }
        if let Some(tx) = self.tx.as_mut() {
            if ends_with(stack, &["DrctDbtTxInf", "DbtrAgt", "FinInstnId", "BICFI"])
                || ends_with(stack, &["DrctDbtTxInf", "DbtrAgt", "FinInstnId", "BIC"])
            {
                tx.debtor_bic = Some(trimmed.to_string());
            }
            if ends_with(stack, &["DrctDbtTxInf", "DbtrAcct", "Id", "IBAN"]) {
                tx.debtor_iban = Some(trimmed.to_string());
            }
        }
    }

    /// Rules on single values: names, amounts, the creditor reference, the
    /// creditor identifier's scheme and the direct debit charge bearer.
    fn content(&mut self, stack: &[String], element: &str, value: &str, line: u32) {
        let parent = stack.len().checked_sub(2).map(|i| stack[i].as_str());
        match (parent, element) {
            (Some(party), "Nm") if PARTIES.contains(&party) => {
                let length = value.chars().count();
                if length > 70 {
                    self.repeat(
                        "name-length",
                        Group::AmountsNames,
                        line,
                        || format!("{party}/Nm has {length} characters; DK allows at most 70"),
                        "Shorten the name to 70 characters. The ISO schema allows 140, but the \
                         DK schemas and banks don't.",
                    );
                }
            }
            (_, "InstdAmt") => match amount_problem(value) {
                Some(Amount::Decimals) => self.repeat(
                    "amount-decimals",
                    Group::AmountsNames,
                    line,
                    || format!("InstdAmt {value} has more than 2 decimal places"),
                    "SEPA amounts have at most 2 decimal places, e.g. 10.50.",
                ),
                Some(Amount::Range) => self.repeat(
                    "amount-range",
                    Group::AmountsNames,
                    line,
                    || format!("InstdAmt {value} is outside 0.01 to 999999999.99"),
                    "A SEPA amount is at least 0.01 and at most 999,999,999.99 EUR.",
                ),
                None => {}
            },
            (Some("CdOrPrtry"), "Cd" | "Prtry")
                if ends_with(stack, &["CdtrRefInf", "Tp", "CdOrPrtry", element])
                    && !(element == "Cd" && value == "SCOR") =>
            {
                self.repeat(
                    "reference-type",
                    Group::AddressesRemittance,
                    line,
                    || {
                        format!(
                            "CdtrRefInf type {element} '{value}' is not allowed; only Cd SCOR is"
                        )
                    },
                    "A structured creditor reference has the type Cd SCOR.",
                );
            }
            (Some("CdtrRefInf"), "Ref")
                if value.len() >= 2
                    && value[..2].eq_ignore_ascii_case("RF")
                    && !rf_reference_is_valid(value) =>
            {
                self.repeat_as(
                    "rf-reference",
                    Group::AddressesRemittance,
                    (Severity::Warning, "ISO 11649"),
                    line,
                    || format!("Creditor reference '{value}' has wrong RF check digits or format"),
                    "An RF creditor reference is RF, 2 check digits (MOD 97-10) and up to 21 \
                     letters or digits. With wrong check digits the payee may not match the payment.",
                );
            }
            (Some("SchmeNm"), "Prtry") if is_creditor_scheme_name(stack) && value != "SEPA" => {
                self.repeat(
                    "scheme-name",
                    Group::Creditor,
                    line,
                    || format!("Creditor identifier scheme '{value}' must be SEPA"),
                    "Set SchmeNm/Prtry of the creditor identifier to SEPA.",
                );
            }
            (_, "ChrgBr") if self.kind == Kind::DirectDebit && value != "SLEV" => {
                self.repeat(
                    "charge-bearer",
                    Group::PaymentType,
                    line,
                    || {
                        format!(
                            "ChrgBr '{value}' is not allowed in a SEPA direct debit; only SLEV is"
                        )
                    },
                    "Use SLEV or leave ChrgBr out.",
                );
            }
            _ => {}
        }
    }

    /// DK Anlage 3, "Zeichensatz": the Latin character set, plus the umlauts,
    /// ß, & * $ % that banks accept. References and IDs: the Latin set only.
    fn characters(&mut self, element: &str, value: &str, is_id: bool, line: u32) {
        let mut bad: Vec<char> = Vec::new();
        for c in value.chars() {
            let ok = c.is_ascii_alphanumeric()
                || " '():?,-+./".contains(c)
                || (!is_id && "ÄÖÜäöüß&*$%".contains(c));
            if !ok && !bad.contains(&c) {
                bad.push(c);
            }
        }
        if bad.is_empty() {
            return;
        }
        let mut list: Vec<String> = bad.iter().take(5).map(|c| format!("{c:?}")).collect();
        if bad.len() > 5 {
            list.push("…".into());
        }
        let list = list.join(", ");
        let (text, hint) = if is_id {
            (
                format!("{element} contains characters not allowed in IDs: {list} (DK rules)."),
                ID_CHARS_HINT,
            )
        } else {
            (
                format!("{element} contains characters outside the DK character set: {list} (DK rules)."),
                CHARS_HINT,
            )
        };
        error(&mut self.out, Group::Characters, line, text, hint);
    }

    /// EPC230-15: references and IDs must not start or end with "/" or contain "//".
    fn slashes(&mut self, element: &str, value: &str, line: u32) {
        if value.starts_with('/') || value.ends_with('/') || value.contains("//") {
            error(
                &mut self.out,
                Group::References,
                line,
                format!("{element} '{value}' must not start or end with '/' or contain '//' (DK rules)."),
                "This EPC rule (EPC230-15) applies to all references and IDs. Banks may reject the file.",
            );
        }
    }

    /// pain.001: no LclInstrm for SEPA credit transfers, only INST for instant
    /// ones. pain.008: CORE or B2B, never both in one file.
    fn local_instrument(&mut self, element: &str, value: &str, line: u32) {
        if self.kind == Kind::CreditTransfer {
            if !(element == "Cd" && value == "INST") {
                error(
                    &mut self.out,
                    Group::PaymentType,
                    line,
                    format!(
                        "LclInstrm {element} '{value}' is not allowed in a SEPA credit transfer; \
                         only Cd INST (instant payment) is (DK rules)."
                    ),
                    "Leave out LclInstrm for a normal SEPA credit transfer.",
                );
            }
            return;
        }
        if element != "Cd" || !matches!(value, "CORE" | "B2B") {
            error(
                &mut self.out,
                Group::PaymentType,
                line,
                format!("LclInstrm {element} '{value}' is not allowed; a SEPA direct debit is Cd CORE or B2B (DK rules)."),
                "Use CORE for SEPA core direct debits and B2B for business-to-business ones.",
            );
            return;
        }
        match &self.first_type {
            None => self.first_type = Some((value.to_string(), line)),
            Some((first, first_line)) if first != value && !self.mixing_reported => {
                let text = format!(
                    "LclInstrm {value} is mixed with {first} (line {first_line}); \
                     one pain.008 must not mix CORE and B2B direct debits (DK rules)."
                );
                self.mixing_reported = true;
                error(
                    &mut self.out,
                    Group::PaymentType,
                    line,
                    text,
                    "Put CORE and B2B direct debits in separate files.",
                );
            }
            Some(_) => {}
        }
    }

    /// DK Anlage 3, "Creditor Identifier (CI)": format and MOD 97-10 check digits.
    fn creditor_id(&mut self, ci: &str, line: u32) {
        if creditor_id_is_valid(ci) || !self.bad_creditor_ids.insert(ci.to_string()) {
            return;
        }
        error(
            &mut self.out,
            Group::Creditor,
            line,
            format!("Creditor identifier '{ci}' has a wrong format or check digits (DK rules)."),
            "A CI is the country code, 2 check digits (MOD 97-10 over the national part and the \
             country code), a 3-character business code of letters or digits (usually ZZZ) and \
             the national ID, e.g. DE98ZZZ09999999999.",
        );
    }

    fn transaction(&mut self, tx: Tx) {
        let direct_debit = self.kind == Kind::DirectDebit;
        let block = &mut self.block;
        match (block.payment_type, tx.payment_type) {
            (true, Some(line)) => block.both_types.add(line),
            (false, None) if direct_debit => block.no_type.add(tx.line),
            _ => {}
        }
        if let (true, Some(line)) = (block.ultimate, tx.ultimate) {
            block.both_ultimates.add(line);
        }
        if let (true, Some(line)) = (block.charge_bearer, tx.charge_bearer) {
            block.both_charges.add(line);
        }
        if !direct_debit {
            return;
        }
        match (block.scheme_id, tx.scheme_id) {
            (true, Some(line)) => block.both_schemes.add(line),
            (false, None) => block.no_scheme.add(tx.line),
            _ => {}
        }
        self.debtor_address(&tx);
    }

    /// DK Anlage 3, Dbtr/PstlAdr: required when the debtor's bank is outside
    /// the EU/EEA. The bank's country is the BIC's, else the IBAN's.
    fn debtor_address(&mut self, tx: &Tx) {
        let country = tx
            .debtor_bic
            .as_deref()
            .and_then(|b| b.get(4..6))
            .or_else(|| tx.debtor_iban.as_deref().and_then(|i| i.get(..2)))
            .map(str::to_ascii_uppercase);
        let Some(country) = country else {
            return;
        };
        if tx.debtor_address || EEA.contains(&country.as_str()) {
            return;
        }
        error(
            &mut self.out,
            Group::AddressesRemittance,
            tx.debtor.unwrap_or(tx.line),
            format!("Dbtr has no PstlAdr, but the debtor's bank is outside the EU/EEA ({country}) (DK rules)."),
            "Direct debits from accounts at banks outside the EU/EEA (e.g. CH, GB) need the \
             debtor's postal address in Dbtr/PstlAdr.",
        );
    }
}

fn is_local_instrument(stack: &[String]) -> bool {
    ends_with(stack, &["PmtTpInf", "LclInstrm", "Cd"])
        || ends_with(stack, &["PmtTpInf", "LclInstrm", "Prtry"])
}

fn is_creditor_id(stack: &[String]) -> bool {
    let tail = ["Id", "PrvtId", "Othr", "Id"];
    stack.len() > tail.len()
        && ends_with(stack, &tail)
        && matches!(
            stack[stack.len() - tail.len() - 1].as_str(),
            "CdtrSchmeId" | "OrgnlCdtrSchmeId"
        )
}

/// SchmeNm/Prtry of a creditor identifier (CdtrSchmeId or OrgnlCdtrSchmeId).
fn is_creditor_scheme_name(stack: &[String]) -> bool {
    let tail = ["Id", "PrvtId", "Othr", "SchmeNm", "Prtry"];
    stack.len() > tail.len()
        && ends_with(stack, &tail)
        && matches!(
            stack[stack.len() - tail.len() - 1].as_str(),
            "CdtrSchmeId" | "OrgnlCdtrSchmeId"
        )
}

enum Amount {
    Decimals,
    Range,
}

/// DK Anlage 3: SEPA amounts have at most 2 decimal places and lie between
/// 0.01 and 999999999.99. Values that aren't plain decimals are left to the XSD.
fn amount_problem(value: &str) -> Option<Amount> {
    let (int, frac) = value.split_once('.').unwrap_or((value, ""));
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if int.is_empty() && frac.is_empty() || !digits(int) || !digits(frac) {
        return None;
    }
    if frac.len() > 2 {
        return Some(Amount::Decimals);
    }
    let int = int.trim_start_matches('0');
    let zero = int.is_empty() && frac.bytes().all(|b| b == b'0');
    (int.len() > 9 || zero).then_some(Amount::Range)
}

/// ISO 11649 creditor reference: RF, 2 check digits, 1 to 21 letters or
/// digits, with the same MOD 97-10 check as an IBAN.
fn rf_reference_is_valid(reference: &str) -> bool {
    let r = reference.as_bytes();
    (5..=25).contains(&r.len())
        && r[2..4].iter().all(u8::is_ascii_digit)
        && r.iter().all(u8::is_ascii_alphanumeric)
        && iban_is_valid(&reference.to_ascii_uppercase())
}

/// Creditor identifier: country code, 2 check digits, 3 alphanumeric business
/// code characters (not part of the check), then the national ID. The check
/// digits are those of an IBAN over the national ID's letters and digits.
fn creditor_id_is_valid(ci: &str) -> bool {
    let b = ci.as_bytes();
    if !(8..=35).contains(&b.len()) || !ci.is_ascii() {
        return false;
    }
    if !b[..2].iter().all(u8::is_ascii_alphabetic)
        || !b[2..4].iter().all(u8::is_ascii_digit)
        || !b[4..7].iter().all(u8::is_ascii_alphanumeric)
    {
        return false;
    }
    let national: String = ci[7..]
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    !national.is_empty() && iban_is_valid(&format!("{}{national}", &ci[..4]))
}

/// The first offending transaction of a payment block, and how many followed.
#[derive(Default)]
struct Tally {
    first: Option<u32>,
    more: u32,
}

impl Tally {
    fn add(&mut self, line: u32) {
        if self.first.is_none() {
            self.first = Some(line);
        } else {
            self.more += 1;
        }
    }

    fn report(self, out: &mut Findings, text: &str, hint: &str) {
        let Some(line) = self.first else {
            return;
        };
        let text = match self.more {
            0 => format!("{text} (DK rules)."),
            1 => format!("{text} (DK rules; 1 more transaction in this payment block too)."),
            n => format!("{text} (DK rules; {n} more transactions in this payment block too)."),
        };
        error(out, Group::PaymentType, line, text, hint);
    }
}

/// One <PmtInf>: what it gives for all its transactions, and the findings.
#[derive(Default)]
struct Block {
    payment_type: bool,
    scheme_id: bool,
    /// UltmtDbtr (pain.001) or UltmtCdtr (pain.008) in PmtInf.
    ultimate: bool,
    charge_bearer: bool,
    both_types: Tally,
    no_type: Tally,
    both_schemes: Tally,
    no_scheme: Tally,
    both_ultimates: Tally,
    both_charges: Tally,
}

impl Block {
    /// DK Anlage 3: PmtTpInf (pain.001, pain.008) and CdtrSchmeId (pain.008)
    /// either in PmtInf or in the transaction, not on both levels. The same
    /// holds for the ultimate debtor (pain.001) or creditor (pain.008) and
    /// for ChrgBr.
    fn finish(self, out: &mut Findings, ultimate: &str) {
        self.both_ultimates.report(
            out,
            &format!("{ultimate} is given both in PmtInf and in the transaction"),
            "Give it once in PmtInf for all transactions or in each transaction, not on both levels.",
        );
        self.both_charges.report(
            out,
            "ChrgBr is given both in PmtInf and in the transaction",
            "Give ChrgBr (SLEV) once in PmtInf (recommended) or in each transaction, not on both levels.",
        );
        self.both_types.report(
            out,
            "PmtTpInf is given both in PmtInf and in the transaction",
            "Give PmtTpInf once in PmtInf (recommended) or in each transaction, not on both levels.",
        );
        self.no_type.report(
            out,
            "PmtTpInf is missing: give it in PmtInf or in the transaction",
            "A direct debit needs PmtTpInf (service level, CORE/B2B, sequence type) in PmtInf \
             or in each transaction.",
        );
        self.both_schemes.report(
            out,
            "CdtrSchmeId is given both in PmtInf and in the transaction",
            "Give the creditor identifier once in PmtInf or in each transaction's DrctDbtTx, \
             not on both levels.",
        );
        self.no_scheme.report(
            out,
            "CdtrSchmeId is missing: give it in PmtInf or in the transaction's DrctDbtTx",
            "Every direct debit needs the creditor identifier (CI), in PmtInf or in each \
             transaction's DrctDbtTx.",
        );
    }
}

/// One <CdtTrfTxInf> or <DrctDbtTxInf>.
#[derive(Default)]
struct Tx {
    line: u32,
    payment_type: Option<u32>,
    scheme_id: Option<u32>,
    debtor: Option<u32>,
    debtor_address: bool,
    debtor_bic: Option<String>,
    debtor_iban: Option<String>,
    ultimate: Option<u32>,
    charge_bearer: Option<u32>,
}

/// A finding reported once per file, with how often it repeated.
struct Repeated {
    key: &'static str,
    group: Group,
    severity: Severity,
    source: &'static str,
    line: u32,
    text: String,
    hint: &'static str,
    more: u32,
}

/// One <RmtInf>: the lines of its Ustrd and Strd elements.
#[derive(Default)]
struct Remittance {
    unstructured: Vec<u32>,
    structured: Vec<u32>,
}

/// One <Strd>: its line and the length of its content with the inner tags.
struct Structured {
    line: u32,
    length: usize,
}

/// One <PstlAdr> of a party.
#[derive(Default)]
struct Address {
    owner: String,
    line: u32,
    town: bool,
    country: bool,
    lines: u32,
}

/// One <MndtRltdInf>.
#[derive(Default)]
struct Mandate {
    /// Line of <AmdmntInd>true</AmdmntInd>.
    amendment: Option<u32>,
    details: bool,
}

impl Mandate {
    /// DK Anlage 3: AmdmntInfDtls is required if AmdmntInd is true.
    fn finish(self, out: &mut Findings) {
        if let (Some(line), false) = (self.amendment, self.details) {
            error(
                out,
                Group::Creditor,
                line,
                "AmdmntInd is true, but AmdmntInfDtls is missing (DK rules).".into(),
                "Say in AmdmntInfDtls what changed (e.g. OrgnlMndtId, or SMNDA in OrgnlDbtrAcct \
                 for a new debtor account), or set AmdmntInd to false.",
            );
        }
    }
}

/// One <AmdmntInfDtls> block of a direct debit mandate.
#[derive(Default)]
struct Amendment {
    debtor_account: bool,
    smnda: bool,
    debtor_agent: Option<u32>,
}

impl Amendment {
    fn start(&mut self, name: &str, stack: &[String], line: u32) {
        if stack.last().map(String::as_str) != Some("AmdmntInfDtls") {
            return;
        }
        match name {
            "OrgnlDbtrAcct" => self.debtor_account = true,
            "OrgnlDbtrAgt" => self.debtor_agent = Some(line),
            _ => {}
        }
    }

    fn text(&mut self, stack: &[String], value: &str) {
        if ends_with(stack, &["OrgnlDbtrAcct", "Id", "Othr", "Id"]) && value.trim() == "SMNDA" {
            self.smnda = true;
        }
    }

    /// DK Anlage 3 V3.9, AmendmentInformationDetails: OrgnlDbtrAgt may only be
    /// given if OrgnlDbtrAcct isn't SMNDA, and an old IBAN in OrgnlDbtrAcct is
    /// only allowed when the debtor's bank stayed the same.
    fn finish(self, out: &mut Findings) {
        let (true, Some(line)) = (self.debtor_account, self.debtor_agent) else {
            return;
        };
        let text = if self.smnda {
            "AmdmntInfDtls: OrgnlDbtrAgt must not be filled when OrgnlDbtrAcct is SMNDA (DK rules)."
        } else {
            "AmdmntInfDtls: OrgnlDbtrAcct and OrgnlDbtrAgt must not both be filled (DK rules)."
        };
        error(
            out,
            Group::Creditor,
            line,
            text.into(),
            "For a new debtor account, put only SMNDA in OrgnlDbtrAcct and leave out OrgnlDbtrAgt. \
             The schema accepts both, but banks following the DK rules reject the file.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A formatted SEPA direct debit (pain.008.001.08) that follows the DK rules.
    const DD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.08">
  <CstmrDrctDbtInitn>
    <GrpHdr>
      <MsgId>MSG-1</MsgId>
    </GrpHdr>
    <PmtInf>
      <PmtInfId>PMT-1</PmtInfId>
      <PmtTpInf>
        <SvcLvl>
          <Cd>SEPA</Cd>
        </SvcLvl>
        <LclInstrm>
          <Cd>CORE</Cd>
        </LclInstrm>
        <SeqTp>RCUR</SeqTp>
      </PmtTpInf>
      <CdtrAcct>
        <Id>
          <IBAN>DE89370400440532013000</IBAN>
        </Id>
      </CdtrAcct>
      <CdtrSchmeId>
        <Id>
          <PrvtId>
            <Othr>
              <Id>DE98ZZZ09999999999</Id>
              <SchmeNm>
                <Prtry>SEPA</Prtry>
              </SchmeNm>
            </Othr>
          </PrvtId>
        </Id>
      </CdtrSchmeId>
      <DrctDbtTxInf>
        <PmtId>
          <EndToEndId>E2E-1</EndToEndId>
        </PmtId>
        <DrctDbtTx>
          <MndtRltdInf>
            <MndtId>M-1</MndtId>
            <DtOfSgntr>2024-01-15</DtOfSgntr>
          </MndtRltdInf>
        </DrctDbtTx>
        <DbtrAgt>
          <FinInstnId>
            <BICFI>COBADEFFXXX</BICFI>
          </FinInstnId>
        </DbtrAgt>
        <Dbtr>
          <Nm>Müller &amp; Söhne *$%</Nm>
        </Dbtr>
        <DbtrAcct>
          <Id>
            <IBAN>DE02120300000000202051</IBAN>
          </Id>
        </DbtrAcct>
      </DrctDbtTxInf>
    </PmtInf>
  </CstmrDrctDbtInitn>
</Document>
"#;

    /// A formatted SEPA credit transfer (pain.001.001.09) that follows the DK rules.
    const CT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.001.001.09">
  <CstmrCdtTrfInitn>
    <GrpHdr>
      <MsgId>MSG-1</MsgId>
    </GrpHdr>
    <PmtInf>
      <PmtInfId>PMT-1</PmtInfId>
      <PmtTpInf>
        <SvcLvl>
          <Cd>SEPA</Cd>
        </SvcLvl>
      </PmtTpInf>
      <DbtrAcct>
        <Id>
          <IBAN>DE89370400440532013000</IBAN>
        </Id>
      </DbtrAcct>
      <ChrgBr>SLEV</ChrgBr>
      <CdtTrfTxInf>
        <PmtId>
          <EndToEndId>E2E-1</EndToEndId>
        </PmtId>
        <Amt>
          <InstdAmt Ccy="EUR">10.00</InstdAmt>
        </Amt>
        <Cdtr>
          <Nm>Müller &amp; Söhne</Nm>
        </Cdtr>
        <RmtInf>
          <Ustrd>Invoice 1/2026 (paid: 50%)</Ustrd>
        </RmtInf>
      </CdtTrfTxInf>
    </PmtInf>
  </CstmrCdtTrfInitn>
</Document>
"#;

    const DD_TYPE: &str = "      <PmtTpInf>
        <SvcLvl>
          <Cd>SEPA</Cd>
        </SvcLvl>
        <LclInstrm>
          <Cd>CORE</Cd>
        </LclInstrm>
        <SeqTp>RCUR</SeqTp>
      </PmtTpInf>
";
    const DD_SCHEME: &str = "      <CdtrSchmeId>
        <Id>
          <PrvtId>
            <Othr>
              <Id>DE98ZZZ09999999999</Id>
              <SchmeNm>
                <Prtry>SEPA</Prtry>
              </SchmeNm>
            </Othr>
          </PrvtId>
        </Id>
      </CdtrSchmeId>
";
    const DEBTOR_NAME: &str = "          <Nm>Müller &amp; Söhne *$%</Nm>\n";

    fn line_of(xml: &str, needle: &str) -> u32 {
        xml.lines().position(|l| l.contains(needle)).unwrap() as u32 + 1
    }

    fn last_line_of(xml: &str, needle: &str) -> u32 {
        let lines: Vec<&str> = xml.lines().collect();
        lines.iter().rposition(|l| l.contains(needle)).unwrap() as u32 + 1
    }

    /// Copies the block that starts with `open` (an indented start tag) and puts
    /// the copy, changed by `change`, right after it.
    fn duplicate(xml: &str, open: &str, change: impl Fn(&str) -> String) -> String {
        let close = format!("{}\n", open.replacen('<', "</", 1));
        let start = xml.find(open).unwrap();
        let end = xml[start..].find(&close).unwrap() + start + close.len();
        format!("{}{}{}", &xml[..end], change(&xml[start..end]), &xml[end..])
    }

    fn two_transactions(xml: &str) -> String {
        duplicate(xml, "      <DrctDbtTxInf>", |tx| {
            tx.replace("E2E-1", "E2E-2")
        })
    }

    fn errors(xml: &str) -> Vec<Message> {
        let out = check(xml);
        assert!(out.iter().all(|m| m.severity == Severity::Error), "{out:?}");
        assert!(out.iter().all(|m| m.hint.is_some()), "{out:?}");
        out
    }

    /// One finding, on `line`, whose text contains `needle`.
    fn assert_one(xml: &str, line: u32, needle: &str) {
        let out = errors(xml);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].line, Some(line), "{out:?}");
        assert!(out[0].text.contains(needle), "{}", out[0].text);
    }

    #[test]
    fn files_that_follow_the_rules_have_no_findings() {
        assert_eq!(check(DD), vec![]);
        assert_eq!(check(CT), vec![]);
        assert_eq!(check(&two_transactions(DD)), vec![]);
    }

    #[test]
    fn swiss_foreign_urgent_and_other_files_are_skipped() {
        let bad = |xml: &str| xml.replace("Invoice 1/2026", "Invoice_1");
        assert_eq!(check(&bad(CT)).len(), 1);
        let swiss = bad(CT).replace("DE89370400440532013000", "CH9300762011623852957");
        assert_eq!(check(&swiss), vec![]);
        assert_eq!(
            check(&bad(CT).replace("Ccy=\"EUR\"", "Ccy=\"USD\"")),
            vec![]
        );
        assert_eq!(check(&bad(CT).replace("SLEV", "SHAR")), vec![]);
        let camt = bad(CT).replace("pain.001.001.09", "camt.053.001.08");
        assert_eq!(check(&camt), vec![]);
    }

    #[test]
    fn characters_outside_the_dk_set_are_errors() {
        let xml = CT.replace("Invoice 1/2026", "Café_Bar @home");
        assert_one(&xml, line_of(&xml, "<Ustrd>"), "'é', '_', '@'");
    }

    #[test]
    fn ids_allow_no_umlauts_or_ampersands() {
        let xml = DD.replace("E2E-1", "E2E-Ä&amp;");
        assert_one(&xml, line_of(&xml, "<EndToEndId>"), "EndToEndId");
        assert!(errors(&xml)[0].text.contains("'Ä', '&'"));
    }

    #[test]
    fn namespace_prefixes_are_one_error() {
        let xml = DD
            .replace("</", "\u{0}")
            .replace('<', "<p:")
            .replace('\u{0}', "</p:")
            .replace("<p:?xml", "<?xml")
            .replace("xmlns=", "xmlns:p=");
        assert_one(&xml, 2, "<p:Document> has a namespace prefix");
    }

    #[test]
    fn ids_must_not_start_or_end_with_a_slash_or_contain_two() {
        for id in ["/E2E", "E2E/", "E2E//1"] {
            let xml = DD.replace("E2E-1", id);
            assert_one(&xml, line_of(&xml, "<EndToEndId>"), &format!("'{id}'"));
        }
        assert_eq!(check(&DD.replace("E2E-1", "E2E/1")), vec![]);
        let xml = DD.replace("MSG-1", "MSG/");
        assert_one(&xml, line_of(&xml, "<MsgId>"), "MsgId");
    }

    #[test]
    fn credit_transfers_take_no_local_instrument_but_inst() {
        let with = |li: &str| {
            CT.replace(
                "        </SvcLvl>\n",
                &format!("        </SvcLvl>\n        <LclInstrm>\n          {li}\n        </LclInstrm>\n"),
            )
        };
        assert_eq!(check(&with("<Cd>INST</Cd>")), vec![]);
        let xml = with("<Prtry>XYZ</Prtry>");
        assert_one(&xml, line_of(&xml, "<Prtry>"), "'XYZ'");
        let xml = with("<Cd>CORE</Cd>");
        assert_one(&xml, line_of(&xml, "<Cd>CORE"), "'CORE'");
    }

    #[test]
    fn direct_debits_must_not_mix_core_and_b2b() {
        let xml = duplicate(DD, "    <PmtInf>", |b| b.replace("CORE", "B2B"));
        assert_one(&xml, line_of(&xml, "<Cd>B2B"), "mixed with CORE");
        let xml = DD.replace("CORE", "COR1");
        assert_one(&xml, line_of(&xml, "<Cd>COR1"), "'COR1'");
    }

    #[test]
    fn direct_debits_are_sepa() {
        let xml = DD.replace("<Cd>SEPA</Cd>", "<Cd>NURG</Cd>");
        assert_one(&xml, line_of(&xml, "<Cd>NURG"), "'NURG'");
    }

    #[test]
    fn payment_type_is_on_exactly_one_level() {
        let in_tx = |xml: &str| {
            xml.replace(
                "        </PmtId>\n",
                &format!("        </PmtId>\n{DD_TYPE}"),
            )
        };
        // Transaction level only: fine.
        assert_eq!(check(&in_tx(&DD.replace(DD_TYPE, ""))), vec![]);
        // Both levels: at the transaction's PmtTpInf.
        let xml = in_tx(DD);
        assert_one(
            &xml,
            last_line_of(&xml, "<PmtTpInf>"),
            "both in PmtInf and in the transaction",
        );
        // Neither: at the transaction, once per block.
        let xml = two_transactions(&DD.replace(DD_TYPE, ""));
        assert_one(&xml, line_of(&xml, "<DrctDbtTxInf>"), "1 more transaction");
        // Credit transfers may leave it out, but not give it twice.
        let ct_type = "      <PmtTpInf>\n        <SvcLvl>\n          <Cd>SEPA</Cd>\n        </SvcLvl>\n      </PmtTpInf>\n";
        assert_eq!(check(&CT.replace(ct_type, "")), vec![]);
        let xml = CT.replace(
            "        </PmtId>\n",
            &format!("        </PmtId>\n{ct_type}"),
        );
        assert_one(&xml, last_line_of(&xml, "<PmtTpInf>"), "both in PmtInf");
    }

    #[test]
    fn creditor_identifier_is_on_exactly_one_level() {
        let in_tx = |xml: &str| {
            xml.replace(
                "          </MndtRltdInf>\n",
                &format!("          </MndtRltdInf>\n{DD_SCHEME}"),
            )
        };
        assert_eq!(check(&in_tx(&DD.replace(DD_SCHEME, ""))), vec![]);
        let xml = in_tx(DD);
        assert_one(
            &xml,
            last_line_of(&xml, "<CdtrSchmeId>"),
            "CdtrSchmeId is given both",
        );
        let xml = DD.replace(DD_SCHEME, "");
        assert_one(
            &xml,
            line_of(&xml, "<DrctDbtTxInf>"),
            "CdtrSchmeId is missing",
        );
    }

    #[test]
    fn creditor_identifiers_are_checked() {
        assert!(creditor_id_is_valid("DE98ZZZ09999999999"));
        assert!(creditor_id_is_valid("DE98ABC09999999999")); // business code isn't checked
        assert!(!creditor_id_is_valid("DE99ZZZ09999999999"));
        assert!(!creditor_id_is_valid("DE98Z_Z09999999999"));
        assert!(!creditor_id_is_valid("DE98ZZZ"));
        let xml = DD.replace("DE98ZZZ09999999999", "DE99ZZZ09999999999");
        assert_one(&xml, line_of(&xml, "DE99ZZZ"), "'DE99ZZZ09999999999'");
        // The same wrong CI in every transaction is reported once.
        let xml = DD.replace(DD_SCHEME, "").replace(
            "          </MndtRltdInf>\n",
            &format!(
                "          </MndtRltdInf>\n{}",
                DD_SCHEME.replace("DE98", "DE99")
            ),
        );
        assert_one(&two_transactions(&xml), line_of(&xml, "DE99ZZZ"), "DE99ZZZ");
    }

    #[test]
    fn amendment_indicator_needs_details() {
        let with = |v: &str| {
            DD.replace(
                "            <DtOfSgntr>2024-01-15</DtOfSgntr>\n",
                &format!("            <DtOfSgntr>2024-01-15</DtOfSgntr>\n            <AmdmntInd>{v}</AmdmntInd>\n"),
            )
        };
        assert_eq!(check(&with("false")), vec![]);
        let xml = with("true");
        assert_one(
            &xml,
            line_of(&xml, "<AmdmntInd>"),
            "AmdmntInfDtls is missing",
        );
    }

    #[test]
    fn debtors_outside_the_eea_need_an_address() {
        let swiss_bank = DD.replace("COBADEFFXXX", "UBSWCHZH80A");
        assert_one(&swiss_bank, line_of(&swiss_bank, "<Dbtr>"), "(CH)");
        let address = "          <PstlAdr>\n            <TwnNm>Zürich</TwnNm>\n            <Ctry>CH</Ctry>\n          </PstlAdr>\n";
        let with_address = swiss_bank.replace(DEBTOR_NAME, &format!("{DEBTOR_NAME}{address}"));
        assert_eq!(check(&with_address), vec![]);
        let xml = DD.replace("COBADEFFXXX", "BARCGB22");
        assert_one(&xml, line_of(&xml, "<Dbtr>"), "(GB)");
        // Without a BIC, the debtor's IBAN tells the country.
        let xml = DD
            .replace(
                "<BICFI>COBADEFFXXX</BICFI>",
                "<Othr><Id>NOTPROVIDED</Id></Othr>",
            )
            .replace("DE02120300000000202051", "CH9300762011623852957");
        assert_one(&xml, line_of(&xml, "<Dbtr>"), "(CH)");
    }

    const CREDITOR_NAME: &str = "          <Nm>Müller &amp; Söhne</Nm>\n";

    /// CT with `extra` right after the creditor's name.
    fn creditor(extra: &str) -> String {
        CT.replace(CREDITOR_NAME, &format!("{CREDITOR_NAME}{extra}"))
    }

    /// CT with `rmt` as the content of RmtInf.
    fn remittance(rmt: &str) -> String {
        CT.replace("          <Ustrd>Invoice 1/2026 (paid: 50%)</Ustrd>\n", rmt)
    }

    #[test]
    fn names_have_at_most_70_characters() {
        let name = |n: usize| format!("          <Nm>{}</Nm>\n", "A".repeat(n));
        assert_eq!(check(&CT.replace(CREDITOR_NAME, &name(70))), vec![]);
        let xml = CT.replace(CREDITOR_NAME, &name(71));
        assert_one(&xml, line_of(&xml, "<Nm>"), "Cdtr/Nm has 71 characters");
        // Repeated: once, with a count.
        let xml = duplicate(&xml, "      <CdtTrfTxInf>", |t| t.replace("E2E-1", "E2E-2"));
        assert_one(&xml, line_of(&xml, "<Nm>"), "1 more in this file");
        // Only party names count.
        let agent = "        <CdtrAgt>\n          <FinInstnId>\n            <Nm>".to_string()
            + &"B".repeat(100)
            + "</Nm>\n          </FinInstnId>\n        </CdtrAgt>\n";
        assert_eq!(
            check(&CT.replace("        <Cdtr>\n", &format!("{agent}        <Cdtr>\n"))),
            vec![]
        );
    }

    #[test]
    fn amounts_have_2_decimals_and_sepa_limits() {
        let amount = |a: &str| CT.replace(">10.00<", &format!(">{a}<"));
        for ok in ["0.01", "10", "10.5", "999999999.99"] {
            assert_eq!(check(&amount(ok)), vec![], "{ok}");
        }
        let xml = amount("10.001");
        assert_one(
            &xml,
            line_of(&xml, "<InstdAmt"),
            "more than 2 decimal places",
        );
        for bad in ["0.00", "0", "1000000000.00"] {
            let xml = amount(bad);
            assert_one(
                &xml,
                line_of(&xml, "<InstdAmt"),
                "outside 0.01 to 999999999.99",
            );
        }
    }

    #[test]
    fn direct_debits_are_in_euro() {
        let xml = DD.replace(
            "        </DrctDbtTx>\n",
            "        </DrctDbtTx>\n        <InstdAmt Ccy=\"USD\">10.00</InstdAmt>\n",
        );
        assert_one(&xml, line_of(&xml, "<InstdAmt"), "in USD");
        assert_eq!(check(&xml.replace("USD", "EUR")), vec![]);
    }

    #[test]
    fn addresses_have_town_and_country_from_iso_2019_on() {
        let address = |inner: &str| {
            creditor(&format!(
                "          <PstlAdr>\n{inner}          </PstlAdr>\n"
            ))
        };
        let town = "            <TwnNm>Berlin</TwnNm>\n";
        let country = "            <Ctry>DE</Ctry>\n";
        let line = "            <AdrLine>Hauptstr. 1</AdrLine>\n";
        assert_eq!(
            check(&address(&format!("{town}{country}{line}{line}"))),
            vec![]
        );
        let xml = address(&format!("{line}{line}"));
        assert_one(
            &xml,
            line_of(&xml, "<PstlAdr>"),
            "Cdtr/PstlAdr has no TwnNm and Ctry",
        );
        let xml = address(country);
        assert_one(&xml, line_of(&xml, "<PstlAdr>"), "has no TwnNm");
        let xml = address(&format!("{town}{country}{line}{line}{line}"));
        assert_one(&xml, line_of(&xml, "<PstlAdr>"), "3 AdrLine");
        // Older versions allow unstructured addresses.
        let old = address(&format!("{line}{line}")).replace("pain.001.001.09", "pain.001.001.03");
        assert_eq!(check(&old), vec![]);
    }

    #[test]
    fn remittance_is_one_ustrd_or_one_strd() {
        let ustrd = "          <Ustrd>Invoice 1</Ustrd>\n";
        let strd = "          <Strd>\n            <CdtrRefInf>\n              <Tp>\n                <CdOrPrtry>\n                  <Cd>SCOR</Cd>\n                </CdOrPrtry>\n              </Tp>\n              <Ref>RF18539007547034</Ref>\n            </CdtrRefInf>\n          </Strd>\n";
        assert_eq!(check(&remittance(strd)), vec![]);
        let xml = remittance(&format!("{ustrd}{ustrd}"));
        assert_one(&xml, last_line_of(&xml, "<Ustrd>"), "more than one Ustrd");
        let xml = remittance(&format!("{strd}{strd}"));
        assert_one(&xml, last_line_of(&xml, "<Strd>"), "more than one Strd");
        let xml = remittance(&format!("{ustrd}{strd}"));
        assert_one(&xml, line_of(&xml, "<Strd>"), "both Ustrd and Strd");
        // Only the creditor reference, of type SCOR.
        let xml = remittance(&strd.replace("SCOR", "RADM"));
        assert_one(&xml, line_of(&xml, "RADM"), "'RADM'");
        let other = "            <RfrdDocInf>\n            </RfrdDocInf>\n";
        let xml = remittance(&strd.replace(
            "          </Strd>\n",
            &format!("{other}          </Strd>\n"),
        ));
        assert_one(
            &xml,
            line_of(&xml, "<RfrdDocInf>"),
            "Strd contains RfrdDocInf",
        );
        // At most 140 characters inside <Strd>, tags included.
        // The inner tags take 81 characters with SCOR, so 59 are left for the reference.
        let xml = remittance(&strd.replace("RF18539007547034", &"X".repeat(59)));
        assert_eq!(check(&xml), vec![], "exactly 140");
        let xml = remittance(&strd.replace("RF18539007547034", &"X".repeat(60)));
        assert_one(
            &xml,
            line_of(&xml, "<Strd>"),
            "141 characters long with its tags",
        );
    }

    #[test]
    fn rf_references_with_wrong_check_digits_are_warnings() {
        assert!(rf_reference_is_valid("RF18539007547034"));
        assert!(rf_reference_is_valid("rf18539007547034"));
        assert!(!rf_reference_is_valid("RF19539007547034"));
        assert!(!rf_reference_is_valid("RF18"));
        let strd = "          <Strd>\n            <CdtrRefInf>\n              <Ref>RF19539007547034</Ref>\n            </CdtrRefInf>\n          </Strd>\n";
        let xml = remittance(strd);
        let out = check(&xml);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].severity, Severity::Warning);
        assert_eq!(out[0].line, Some(line_of(&xml, "<Ref>")));
        assert!(out[0].text.contains("(ISO 11649)"), "{}", out[0].text);
        // Other references aren't RF references.
        assert_eq!(check(&remittance(&strd.replace("RF19", "AB19"))), vec![]);
    }

    #[test]
    fn ultimate_parties_and_charge_bearer_are_on_one_level() {
        let ultimate =
            "        <UltmtDbtr>\n          <Nm>Tochter GmbH</Nm>\n        </UltmtDbtr>\n";
        let in_block = CT.replace(
            "      <ChrgBr>SLEV</ChrgBr>\n",
            &format!(
                "      <ChrgBr>SLEV</ChrgBr>\n{}",
                ultimate.replacen("  ", "", 3)
            ),
        );
        assert_eq!(check(&in_block), vec![]);
        let xml = in_block.replace("        <Cdtr>\n", &format!("{ultimate}        <Cdtr>\n"));
        assert_one(
            &xml,
            last_line_of(&xml, "<UltmtDbtr>"),
            "UltmtDbtr is given both",
        );
        let xml = CT.replace(
            "        <Cdtr>\n",
            "        <ChrgBr>SLEV</ChrgBr>\n        <Cdtr>\n",
        );
        assert_one(&xml, last_line_of(&xml, "<ChrgBr>"), "ChrgBr is given both");
        // Direct debits: UltmtCdtr.
        let xml = DD
            .replace(
                "      <CdtrSchmeId>\n",
                "      <UltmtCdtr>\n        <Nm>X</Nm>\n      </UltmtCdtr>\n      <CdtrSchmeId>\n",
            )
            .replace(
                "        <DbtrAgt>\n",
                "        <UltmtCdtr>\n          <Nm>Y</Nm>\n        </UltmtCdtr>\n        <DbtrAgt>\n",
            );
        assert_one(
            &xml,
            last_line_of(&xml, "<UltmtCdtr>"),
            "UltmtCdtr is given both",
        );
    }

    #[test]
    fn direct_debits_charge_slev_only() {
        let with = |c: &str| {
            DD.replace(
                "      <CdtrSchmeId>\n",
                &format!("      <ChrgBr>{c}</ChrgBr>\n      <CdtrSchmeId>\n"),
            )
        };
        assert_eq!(check(&with("SLEV")), vec![]);
        let xml = with("SHAR");
        assert_one(&xml, line_of(&xml, "<ChrgBr>"), "'SHAR'");
    }

    #[test]
    fn creditor_identifier_scheme_is_sepa() {
        let xml = DD.replace("<Prtry>SEPA</Prtry>", "<Prtry>CORE</Prtry>");
        assert_one(&xml, line_of(&xml, "<Prtry>CORE"), "'CORE' must be SEPA");
    }

    #[test]
    fn disabled_groups_are_skipped() {
        let xml = DD
            .replace("<Cd>SEPA</Cd>", "<Cd>NURG</Cd>")
            .replace("E2E-1", "E2E_1/");
        assert_eq!(errors(&xml).len(), 3);
        let out = check_without(&xml, &[Group::PaymentType, Group::References]);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(
            out[0].text.contains("not allowed in IDs"),
            "{}",
            out[0].text
        );
        let all = [
            Group::Characters,
            Group::References,
            Group::PaymentType,
            Group::Creditor,
            Group::AmountsNames,
            Group::AddressesRemittance,
        ];
        assert_eq!(check_without(&xml, &all), vec![]);
    }

    #[test]
    fn groups_have_stable_names_for_the_settings() {
        let names = r#"["characters","references","payment-type","creditor","amounts-names","addresses-remittance"]"#;
        let groups: Vec<Group> = serde_json::from_str(names).unwrap();
        assert_eq!(groups.len(), 6);
        assert_eq!(serde_json::to_string(&groups).unwrap(), names);
    }

    // DK rule for mandate amendments (AmdmntInfDtls).

    fn pain008(amendment: &str) -> String {
        DD.replace(
            "            <DtOfSgntr>2024-01-15</DtOfSgntr>\n",
            &format!(
                "            <DtOfSgntr>2024-01-15</DtOfSgntr>\n            <AmdmntInd>true</AmdmntInd>\n            <AmdmntInfDtls>\n{amendment}\n            </AmdmntInfDtls>\n"
            ),
        )
    }

    const SMNDA_ACCT: &str = "              <OrgnlDbtrAcct>
                <Id>
                  <Othr>
                    <Id>SMNDA</Id>
                  </Othr>
                </Id>
              </OrgnlDbtrAcct>";
    const IBAN_ACCT: &str = "              <OrgnlDbtrAcct>
                <Id>
                  <IBAN>DE89370400440532013000</IBAN>
                </Id>
              </OrgnlDbtrAcct>";
    const OLD_AGENT: &str = "              <OrgnlDbtrAgt>
                <FinInstnId>
                  <BICFI>COBADEFFXXX</BICFI>
                </FinInstnId>
              </OrgnlDbtrAgt>";

    #[test]
    fn smnda_alone_is_fine() {
        assert_eq!(check(&pain008(SMNDA_ACCT)), vec![]);
    }

    #[test]
    fn an_old_agent_alone_is_fine() {
        assert_eq!(check(&pain008(OLD_AGENT)), vec![]);
    }

    #[test]
    fn smnda_with_an_old_agent_is_an_error_at_the_agent() {
        let xml = pain008(&format!("{SMNDA_ACCT}\n{OLD_AGENT}"));
        assert_one(&xml, line_of(&xml, "<OrgnlDbtrAgt>"), "SMNDA");
    }

    #[test]
    fn an_old_iban_with_an_old_agent_is_an_error_too() {
        let xml = pain008(&format!("{IBAN_ACCT}\n{OLD_AGENT}"));
        let out = errors(&xml);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(!out[0].text.contains("SMNDA"), "{}", out[0].text);
    }

    #[test]
    fn each_amendment_block_is_checked_on_its_own() {
        // Account in the first block, agent in the second: both allowed.
        let xml = pain008(SMNDA_ACCT).replace(
            "            </AmdmntInfDtls>",
            &format!("            </AmdmntInfDtls>\n            <AmdmntInfDtls>\n{OLD_AGENT}\n            </AmdmntInfDtls>"),
        );
        assert_eq!(check(&xml), vec![]);
    }

    #[test]
    fn unformatted_single_line_files_are_checked() {
        let xml = pain008(&format!("{SMNDA_ACCT}\n{OLD_AGENT}"))
            .lines()
            .map(str::trim)
            .collect::<String>();
        let out = errors(&xml);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].line, Some(1));
    }
}
