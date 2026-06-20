//! Per-file SEPA payment summary: creditor + PmtInf block stats + per-transaction
//! remittance info. Read-only extraction with quick-xml; does not affect validation.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;

use crate::validator::detect_namespace;

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Creditor {
    pub name: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub creditor_id: Option<String>,
}

impl Creditor {
    fn has_any(&self) -> bool {
        self.name.is_some() || self.iban.is_some() || self.bic.is_some() || self.creditor_id.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PmtInfSummary {
    pub nb_of_txs: Option<String>,
    pub ctrl_sum: Option<String>,
    pub svc_lvl_cd: Option<String>,
    pub lcl_instrm: Option<String>,
    pub seq_tp: Option<String>,
    pub reqd_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemittanceEntry {
    pub instr_id: Option<String>,
    pub end_to_end_id: Option<String>,
    pub ustrd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentSummary {
    pub message_type: String,
    pub pmt_inf_count: u32,
    pub creditor: Option<Creditor>,
    pub blocks: Vec<PmtInfSummary>,
    pub transactions: Vec<RemittanceEntry>,
}

/// Local element name (namespace prefix stripped) as an owned String.
fn local_of(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    match s.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => s.into_owned(),
    }
}

/// Per-transaction accumulation while parsing (not serialized).
#[derive(Default)]
struct TxAccum {
    ustrd: Vec<String>,
    instr_id: Option<String>,
    end_to_end_id: Option<String>,
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
    let mut transactions: Vec<RemittanceEntry> = Vec::new();
    let mut current: Option<PmtInfSummary> = None;
    let mut current_creditor = Creditor::default();
    let mut creditor: Option<Creditor> = None;
    let mut current_tx: Option<TxAccum> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).map_err(|e| e.to_string())? {
            Event::Start(e) => {
                let name = local_of(e.name().as_ref());
                match name.as_str() {
                    "PmtInf" => {
                        current = Some(PmtInfSummary::default());
                        current_creditor = Creditor::default();
                    }
                    "CdtTrfTxInf" | "DrctDbtTxInf" => current_tx = Some(TxAccum::default()),
                    _ => {}
                }
                stack.push(name);
            }
            Event::End(e) => {
                let name = local_of(e.name().as_ref());
                match name.as_str() {
                    "PmtInf" => {
                        if let Some(b) = current.take() {
                            blocks.push(b);
                        }
                        if creditor.is_none() && current_creditor.has_any() {
                            creditor = Some(std::mem::take(&mut current_creditor));
                        }
                    }
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
                    _ => {}
                }
                stack.pop();
            }
            Event::Text(t) => {
                if current.is_none() {
                    continue;
                }
                let top = stack.last().map(String::as_str).unwrap_or("");
                let parent = stack.iter().rev().nth(1).map(String::as_str).unwrap_or("");
                let grand = stack.iter().rev().nth(2).map(String::as_str).unwrap_or("");
                let text = t.unescape().map_err(|e| e.to_string())?.into_owned();
                let b = current.as_mut().unwrap();
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
                    "Cd" if parent == "LclInstrm" && grand == "PmtTpInf" => {
                        b.lcl_instrm.get_or_insert(text);
                    }
                    "SeqTp" if parent == "PmtTpInf" => {
                        b.seq_tp.get_or_insert(text);
                    }
                    "ReqdExctnDt" | "ReqdColltnDt" => {
                        b.reqd_date.get_or_insert(text);
                    }
                    "Dt" | "DtTm" if parent == "ReqdExctnDt" || parent == "ReqdColltnDt" => {
                        b.reqd_date.get_or_insert(text);
                    }
                    "Nm" if parent == "Cdtr" && current_tx.is_none() => {
                        current_creditor.name.get_or_insert(text);
                    }
                    "IBAN" if parent == "Id" && grand == "CdtrAcct" && current_tx.is_none() => {
                        current_creditor.iban.get_or_insert(text);
                    }
                    "BIC" | "BICFI"
                        if parent == "FinInstnId" && grand == "CdtrAgt" && current_tx.is_none() =>
                    {
                        current_creditor.bic.get_or_insert(text);
                    }
                    "Id" if parent == "Othr"
                        && current_tx.is_none()
                        && stack.iter().any(|s| s == "CdtrSchmeId") =>
                    {
                        current_creditor.creditor_id.get_or_insert(text);
                    }
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
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    Ok(PaymentSummary {
        message_type,
        pmt_inf_count: blocks.len() as u32,
        creditor,
        blocks,
        transactions,
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
      <CdtTrfTxInf><PmtId><InstrId>INSTR-1</InstrId><EndToEndId>E2E-1</EndToEndId></PmtId><Cdtr><Nm>Payee One</Nm></Cdtr><RmtInf><Ustrd>Invoice 1</Ustrd></RmtInf></CdtTrfTxInf>
      <CdtTrfTxInf><PmtId><EndToEndId>E2E-2</EndToEndId></PmtId><Cdtr><Nm>Payee Two</Nm></Cdtr><RmtInf><Ustrd>Invoice 2</Ustrd></RmtInf></CdtTrfTxInf>
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

    #[test]
    fn pain001_blocks_transactions_and_no_pmtinf_creditor() {
        let p = temp_xml("sepa_sum2_p001.xml", PAIN001);
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.message_type, "pain.001.001.03");
        assert_eq!(s.pmt_inf_count, 2);
        // PmtInf-level values, not GrpHdr (3 / 600.00).
        assert_eq!(s.blocks[0].nb_of_txs.as_deref(), Some("2"));
        assert_eq!(s.blocks[0].ctrl_sum.as_deref(), Some("300.00"));
        assert_eq!(s.blocks[0].svc_lvl_cd.as_deref(), Some("SEPA"));
        assert_eq!(s.blocks[0].lcl_instrm, None);
        assert_eq!(s.blocks[0].seq_tp, None);
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-06-20"));
        // pain.001: Cdtr is per transaction, so there is NO PmtInf-level creditor.
        assert_eq!(s.creditor, None);
        // One entry per transaction, document order.
        assert_eq!(s.transactions.len(), 3);
        assert_eq!(s.transactions[0].ustrd.as_deref(), Some("Invoice 1"));
        assert_eq!(s.transactions[1].ustrd.as_deref(), Some("Invoice 2"));
        assert_eq!(s.transactions[2].ustrd.as_deref(), Some("Invoice 3"));
        // Origin: InstrId preferred; tx2 falls back to EndToEndId; tx3 has neither.
        assert_eq!(s.transactions[0].instr_id.as_deref(), Some("INSTR-1"));
        assert_eq!(s.transactions[0].end_to_end_id.as_deref(), Some("E2E-1"));
        assert_eq!(s.transactions[1].instr_id, None);
        assert_eq!(s.transactions[1].end_to_end_id.as_deref(), Some("E2E-2"));
        assert_eq!(s.transactions[2].instr_id, None);
        assert_eq!(s.transactions[2].end_to_end_id, None);
    }

    const PAIN008: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.02">
  <CstmrDrctDbtInitn>
    <GrpHdr><MsgId>M2</MsgId></GrpHdr>
    <PmtInf>
      <NbOfTxs>3</NbOfTxs>
      <CtrlSum>150.00</CtrlSum>
      <PmtTpInf>
        <SvcLvl><Cd>SEPA</Cd></SvcLvl>
        <LclInstrm><Cd>CORE</Cd></LclInstrm>
        <SeqTp>RCUR</SeqTp>
      </PmtTpInf>
      <ReqdColltnDt>2026-07-01</ReqdColltnDt>
      <Cdtr><Nm>ACME GmbH</Nm></Cdtr>
      <CdtrAcct><Id><IBAN>DE89370400440532013000</IBAN></Id></CdtrAcct>
      <CdtrAgt><FinInstnId><BIC>COBADEFFXXX</BIC></FinInstnId></CdtrAgt>
      <CdtrSchmeId><Id><PrvtId><Othr><Id>DE98ZZZ09999999999</Id></Othr></PrvtId></Id></CdtrSchmeId>
      <DrctDbtTxInf><RmtInf><Ustrd>Beitrag Mai</Ustrd></RmtInf></DrctDbtTxInf>
      <DrctDbtTxInf><RmtInf><Ustrd></Ustrd></RmtInf></DrctDbtTxInf>
      <DrctDbtTxInf></DrctDbtTxInf>
    </PmtInf>
  </CstmrDrctDbtInitn>
</Document>"#;

    #[test]
    fn pain008_creditor_lclinstrm_seqtp_and_missing_empty_ustrd() {
        let p = temp_xml("sepa_sum2_p008.xml", PAIN008);
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.message_type, "pain.008.001.02");
        assert_eq!(s.pmt_inf_count, 1);
        assert_eq!(s.blocks[0].lcl_instrm.as_deref(), Some("CORE"));
        assert_eq!(s.blocks[0].seq_tp.as_deref(), Some("RCUR"));
        assert_eq!(s.blocks[0].svc_lvl_cd.as_deref(), Some("SEPA"));
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-07-01"));
        let c = s.creditor.expect("creditor present");
        assert_eq!(c.name.as_deref(), Some("ACME GmbH"));
        assert_eq!(c.iban.as_deref(), Some("DE89370400440532013000"));
        assert_eq!(c.bic.as_deref(), Some("COBADEFFXXX"));
        assert_eq!(c.creditor_id.as_deref(), Some("DE98ZZZ09999999999"));
        assert_eq!(s.transactions.len(), 3);
        assert_eq!(s.transactions[0].ustrd.as_deref(), Some("Beitrag Mai"));
        assert_eq!(s.transactions[1].ustrd, None); // empty <Ustrd></Ustrd>
        assert_eq!(s.transactions[2].ustrd, None); // no RmtInf/Ustrd
    }

    #[test]
    fn nested_reqd_exctn_dt_resolves_inner_date() {
        let p = temp_xml(
            "sepa_sum2_p009.xml",
            r#"<?xml version="1.0"?>
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
</Document>"#,
        );
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.blocks[0].reqd_date.as_deref(), Some("2026-08-15"));
        assert_eq!(s.transactions.len(), 1);
        assert_eq!(s.transactions[0].ustrd.as_deref(), Some("Nested date"));
    }

    #[test]
    fn non_payment_doc_is_empty() {
        let p = temp_xml(
            "sepa_sum2_none.xml",
            r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.002.001.10"><CstmrPmtStsRpt><GrpHdr><MsgId>X</MsgId></GrpHdr></CstmrPmtStsRpt></Document>"#,
        );
        let s = extract_payment_summary(&p).unwrap();
        assert_eq!(s.pmt_inf_count, 0);
        assert!(s.blocks.is_empty());
        assert!(s.transactions.is_empty());
        assert_eq!(s.creditor, None);
    }

    #[test]
    fn malformed_is_err() {
        let p = temp_xml("sepa_sum2_bad.xml", "<a><b></a>");
        assert!(extract_payment_summary(&p).is_err());
    }
}
