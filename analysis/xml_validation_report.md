# SEPA XML Fehlerreport (priorisiert)

Quelle: /tmp/sepa_validation_results.txt

## Zusammenfassung
- Dateien gesamt: 41
- OK: 20
- FAIL: 21
- NO_SCHEMA: 0

## Fehler pro Datei
### 20241016_sepa_ct_zw_c.xml
- Priorität: MEDIUM
- Schema: `xml_schema/pain.001.001.09.xsd`
- Fehler: to_check/20241016_sepa_ct_zw_c.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}BtchBookg': '000' is not a valid value of the atomic type '{urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}BatchBookingIndicator'.
- Fix-Hinweis: BtchBookg auf true/false setzen.

### 20241016_sepa_ct_zw_d.xml
- Priorität: MEDIUM
- Schema: `xml_schema/pain.008.001.08.xsd`
- Fehler: to_check/20241016_sepa_ct_zw_d.xml:26: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}CtryOfRes': This element is not expected.
- Fix-Hinweis: CtryOfRes an dieser Position entfernen/verschieben.

### 20250121_1000_P2S_940_SEPA_CT.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.001.001.09.xsd`
- Fehler: to_check/20250121_1000_P2S_940_SEPA_CT.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}GrpHdr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}InitgPty ).
- Fix-Hinweis: GrpHdr um InitgPty ergänzen (Pflichtfeld).

### 20250121_1000_P2S_940_SEPA_DD.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.08.xsd`
- Fehler: to_check/20250121_1000_P2S_940_SEPA_DD.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}GrpHdr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}InitgPty ).
- Fix-Hinweis: GrpHdr um InitgPty ergänzen (Pflichtfeld).

### 20250121_1000_P2S_940_SEPA_DD_2 (1).xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.08.xsd`
- Fehler: to_check/20250121_1000_P2S_940_SEPA_DD_2 (1).xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}GrpHdr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}InitgPty ).
- Fix-Hinweis: GrpHdr um InitgPty ergänzen (Pflichtfeld).

### 20250121_1000_P2S_940_SEPA_DD_2.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.08.xsd`
- Fehler: to_check/20250121_1000_P2S_940_SEPA_DD_2.xml:5: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}GrpHdr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}InitgPty ).
- Fix-Hinweis: GrpHdr um InitgPty ergänzen (Pflichtfeld).

### 20250121_1000_P2S_940_SEPA_DD_2_DiskStation_Sep-01-1357-2025_DownloadConflict.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.08.xsd`
- Fehler: to_check/20250121_1000_P2S_940_SEPA_DD_2_DiskStation_Sep-01-1357-2025_DownloadConflict.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}GrpHdr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}InitgPty ).
- Fix-Hinweis: GrpHdr um InitgPty ergänzen (Pflichtfeld).

### 20250121_1020_P2D_950_SEPA_CT.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.001.001.09.xsd`
- Fehler: to_check/20250121_1020_P2D_950_SEPA_CT.xml:4: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}GrpHdr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}InitgPty ).
- Fix-Hinweis: GrpHdr um InitgPty ergänzen (Pflichtfeld).

### 20250121_1020_P2D_950_SEPA_DD.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.08.xsd`
- Fehler: to_check/20250121_1020_P2D_950_SEPA_DD.xml:4: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}GrpHdr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.08}InitgPty ).
- Fix-Hinweis: GrpHdr um InitgPty ergänzen (Pflichtfeld).

### 20251218_VELBERT_CT.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.001.001.09.xsd`
- Fehler: to_check/20251218_VELBERT_CT.xml:4: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}GrpHdr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}InitgPty ).
- Fix-Hinweis: GrpHdr um InitgPty ergänzen (Pflichtfeld).

### Q2S_CT_1000_KSK.xml
- Priorität: MEDIUM
- Schema: `xml_schema/pain.001.001.09.xsd`
- Fehler: to_check/Q2S_CT_1000_KSK.xml:47: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}ChrgBr': [facet 'enumeration'] The value 'SHAR' is not an element of the set {'SLEV'}.
- Fix-Hinweis: ChrgBr auf SLEV ändern.

### dd_change_2.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/dd_change_2.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### lk002_dd.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/lk002_dd.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### q2s_dd_1.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/q2s_dd_1.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### q2s_dd_2 (1).xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/q2s_dd_2 (1).xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### q2s_dd_2.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/q2s_dd_2.xml:77369: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### q2s_dd_2_DiskStation_Sep-01-1400-2025_DownloadConflict.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/q2s_dd_2_DiskStation_Sep-01-1400-2025_DownloadConflict.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### q2s_test100 (1).xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/q2s_test100 (1).xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### q2s_test100.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/q2s_test100.xml:19264: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### q2s_test100_DiskStation_Sep-01-1400-2025_DownloadConflict.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/q2s_test100_DiskStation_Sep-01-1400-2025_DownloadConflict.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

### q2s_test200.xml
- Priorität: HIGH
- Schema: `xml_schema/pain.008.001.02.xsd`
- Fehler: to_check/q2s_test200.xml:1: Schemas validity error : Element '{urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}Othr': Missing child element(s). Expected is ( {urn:iso:std:iso:20022:tech:xsd:pain.008.001.02}SchmeNm ).
- Fix-Hinweis: Unter Othr das Pflichtfeld SchmeNm ergänzen.

