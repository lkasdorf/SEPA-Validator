// Plain-language name for the XSD a file was validated against, so the log can
// say which kind of payment the app took the file for (several DK subsets share
// one namespace, e.g. SEPA, urgent euro and foreign pain.001.001.09).
const LABELS: Record<string, string> = {
  "pain.001.001.03.xsd": "SEPA credit transfer (pain.001.001.03)",
  "pain.001.001.09.xsd": "SEPA credit transfer (DK)",
  "pain.001.001.09_AXZ_GBIC_5.xsd": "Foreign payment (DK AXZ)",
  "pain.001.001.09_CCU_GBIC_5.xsd": "Urgent euro transfer (DK CCU)",
  "pain.001.001.09.ch.03.xsd": "Swiss credit transfer (SPS)",
  "pain.002.001.10.xsd": "Payment status report",
  "pain.007.001.09_GBIC_5.xsd": "Direct debit reversal (DK)",
  "pain.008.001.02.xsd": "SEPA direct debit (pain.008.001.02)",
  "pain.008.001.08.xsd": "SEPA direct debit (DK)",
  "pain.008.001.02.ch.03.xsd": "Swiss direct debit (SPS)",
  "pain.008.001.02.chsdd.02.xsd": "Swiss direct debit (SPS, CH-DD)",
  "camt.052.001.08.xsd": "Intraday account report",
  "camt.053.001.08.xsd": "Account statement",
  "camt.054.001.08.xsd": "Debit/credit notification",
  "container.nnn.001.GBIC4.xsd": "DK SEPA container (GBIC4)",
  "container.nnn.001.GBIC5.xsd": "DK SEPA container (GBIC5)",
};

export function schemaLabel(schema: string): string | undefined {
  return LABELS[schema];
}
