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
        buf.clear();
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
    }

    Ok(PaymentSummary {
        message_type,
        pmt_inf_count: blocks.len() as u32,
        blocks,
        ustrd,
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
