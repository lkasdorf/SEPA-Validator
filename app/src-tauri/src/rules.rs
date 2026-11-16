//! Usage rules of the German banking industry (DK, "DFÜ-Abkommen Anlage 3",
//! column "EPC-/DK-Regeln") that the XSD can't express. A file that breaks
//! them is schema-valid, but banks may reject it, so they are errors.

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::model::{Message, Severity};
use crate::plausibility::LineCounter;

/// Check a (formatted) document against the DK usage rules.
pub fn check(xml: &str) -> Vec<Message> {
    let mut out = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut lines = LineCounter::new(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut amendment: Option<Amendment> = None;
    loop {
        let line = lines.at(reader.buffer_position() as usize);
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if let Some(a) = amendment.as_mut() {
                    a.start(&name, &stack, line);
                }
                if name == "AmdmntInfDtls" {
                    amendment = Some(Amendment::default());
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                if let Some(a) = amendment.as_mut() {
                    let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                    a.start(&name, &stack, line);
                }
            }
            Ok(Event::End(_)) => {
                if stack.pop().as_deref() == Some("AmdmntInfDtls") {
                    if let Some(a) = amendment.take() {
                        a.finish(&mut out);
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if let (Some(a), Ok(v)) = (amendment.as_mut(), t.unescape()) {
                    a.text(&stack, &v);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    out
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
        if stack.ends_with(&["OrgnlDbtrAcct", "Id", "Othr", "Id"].map(String::from))
            && value.trim() == "SMNDA"
        {
            self.smnda = true;
        }
    }

    /// DK Anlage 3 V3.9, AmendmentInformationDetails: OrgnlDbtrAgt may only be
    /// given if OrgnlDbtrAcct isn't SMNDA, and an old IBAN in OrgnlDbtrAcct is
    /// only allowed when the debtor's bank stayed the same.
    fn finish(self, out: &mut Vec<Message>) {
        let (true, Some(line)) = (self.debtor_account, self.debtor_agent) else {
            return;
        };
        let text = if self.smnda {
            "AmdmntInfDtls: OrgnlDbtrAgt must not be filled when OrgnlDbtrAcct is SMNDA (DK rules)."
        } else {
            "AmdmntInfDtls: OrgnlDbtrAcct and OrgnlDbtrAgt must not both be filled (DK rules)."
        };
        out.push(Message {
            severity: Severity::Error,
            text: text.into(),
            line: Some(line),
            column: None,
            hint: Some(
                "For a new debtor account, put only SMNDA in OrgnlDbtrAcct and leave out OrgnlDbtrAgt. \
                 The schema accepts both, but banks following the DK rules reject the file."
                    .into(),
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Severity;

    /// A formatted pain.008.001.08 transaction whose mandate block has `amendment`
    /// as the content of <AmdmntInfDtls> (one element per line, like the viewer).
    fn pain008(amendment: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.008.001.08\">
  <CstmrDrctDbtInitn>
    <PmtInf>
      <DrctDbtTxInf>
        <DrctDbtTx>
          <MndtRltdInf>
            <MndtId>M-1</MndtId>
            <DtOfSgntr>2024-01-15</DtOfSgntr>
            <AmdmntInd>true</AmdmntInd>
            <AmdmntInfDtls>
{amendment}
            </AmdmntInfDtls>
          </MndtRltdInf>
        </DrctDbtTx>
      </DrctDbtTxInf>
    </PmtInf>
  </CstmrDrctDbtInitn>
</Document>
"
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

    fn line_of(xml: &str, needle: &str) -> u32 {
        xml.lines().position(|l| l.contains(needle)).unwrap() as u32 + 1
    }

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
        let out = check(&xml);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].severity, Severity::Error);
        assert_eq!(out[0].line, Some(line_of(&xml, "<OrgnlDbtrAgt>")));
        assert!(out[0].text.contains("SMNDA"), "{}", out[0].text);
        assert!(out[0].hint.is_some());
    }

    #[test]
    fn an_old_iban_with_an_old_agent_is_an_error_too() {
        let xml = pain008(&format!("{IBAN_ACCT}\n{OLD_AGENT}"));
        let out = check(&xml);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].severity, Severity::Error);
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
        let out = check(&xml);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].line, Some(1));
    }
}
