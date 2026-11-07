//! Plain-language names for German business transaction codes (GVC), read
//! from the DK mapping table "Anhang 1 zu Anlage 3 - Datenformatstandards"
//! (sheet "GVC ► ISO BTC"). Like the XSDs the table is not bundled: the user
//! imports the .xlsx (or the ZIP it ships in) and a compact JSON copy is kept
//! next to the imported schemas.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};

/// File name of the converted table in the schema dir.
pub const FILE_NAME: &str = "gvc-mapping.json";
/// Largest workbook and largest single part we read (the DK table is ~150 KB).
const MAX_WORKBOOK_BYTES: u64 = 20 * 1024 * 1024;
const MAX_PART_BYTES: u64 = 50 * 1024 * 1024;

/// One row of the mapping: a GVC, its name, and the ISO code and debit/credit
/// side it applies to (several rows per GVC are possible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GvcRow {
    pub gvc: String,
    pub name: String,
    /// "PMNT-RCDT-ESCT"; empty if the row has none.
    pub iso: String,
    /// "C" or "D"; empty if the row has none.
    pub side: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GvcTable {
    /// The workbook the table was imported from.
    pub source: String,
    pub rows: Vec<GvcRow>,
}

impl GvcTable {
    /// Number of distinct GVCs.
    pub fn codes(&self) -> usize {
        let mut seen: Vec<&str> = self.rows.iter().map(|r| r.gvc.as_str()).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    }

    /// Name of `gvc`, preferring the row that also matches the ISO code and
    /// the side ("DBIT"/"CRDT"), then the ISO code, then the side.
    pub fn name(&self, gvc: &str, iso: Option<&str>, credit_debit: Option<&str>) -> Option<&str> {
        let side = match credit_debit {
            Some("DBIT") => "D",
            Some("CRDT") => "C",
            _ => "",
        };
        let rows: Vec<&GvcRow> = self.rows.iter().filter(|r| r.gvc == gvc).collect();
        let iso_ok = |r: &GvcRow| iso.is_some_and(|i| !r.iso.is_empty() && r.iso == i);
        let side_ok = |r: &GvcRow| !side.is_empty() && r.side == side;
        rows.iter()
            .find(|r| iso_ok(r) && side_ok(r))
            .or_else(|| rows.iter().find(|r| iso_ok(r)))
            .or_else(|| rows.iter().find(|r| side_ok(r)))
            .or_else(|| rows.first())
            .map(|r| r.name.as_str())
    }

    pub fn load(dir: &Path) -> Option<GvcTable> {
        let text = std::fs::read_to_string(dir.join(FILE_NAME)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        let tmp = dir.join(format!("{FILE_NAME}.part"));
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, dir.join(FILE_NAME)).map_err(|e| e.to_string())
    }
}

/// Reads the mapping from an .xlsx file's bytes. `source` names it in the table.
pub fn parse_workbook(bytes: &[u8], source: &str) -> Result<GvcTable, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "not an Excel file")?;
    let sheet = gvc_sheet_path(&mut zip)?;
    let strings = match read_part(&mut zip, "xl/sharedStrings.xml") {
        Ok(xml) => shared_strings(&xml),
        Err(_) => Vec::new(),
    };
    let sheet_xml = read_part(&mut zip, &sheet)?;
    let rows = sheet_rows(&sheet_xml, &strings)
        .into_iter()
        .filter_map(|cells| {
            // Numeric cells may come back as "166.0".
            let gvc = cells.first()?.trim().trim_end_matches(".0").to_string();
            let name = cells.get(1)?.trim().to_string();
            if gvc.is_empty() || !gvc.bytes().all(|b| b.is_ascii_digit()) || name.is_empty() {
                return None; // header and section rows ("0XX und 100")
            }
            let col = |i: usize| cells.get(i).map(|s| s.trim()).unwrap_or("");
            let iso_parts: Vec<&str> = [col(4), col(5), col(6)]
                .into_iter()
                .take_while(|p| !p.is_empty())
                .collect();
            Some(GvcRow {
                gvc: format!("{gvc:0>3}"),
                name,
                iso: iso_parts.join("-"),
                side: col(3).to_string(),
            })
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err("no GVC rows found".into());
    }
    Ok(GvcTable {
        source: source.to_string(),
        rows,
    })
}

/// Reads an .xlsx from disk (size-capped).
pub fn parse_workbook_file(path: &Path) -> Result<GvcTable, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    f.take(MAX_WORKBOOK_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_WORKBOOK_BYTES {
        return Err("Excel file too large".into());
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    parse_workbook(&bytes, name)
}

/// Reads an .xlsx entry of a ZIP (size-capped).
pub fn read_capped(entry: &mut impl Read) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    entry
        .take(MAX_WORKBOOK_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_WORKBOOK_BYTES).then_some(bytes)
}

fn read_part(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, String> {
    let entry = zip
        .by_name(name)
        .map_err(|_| format!("{name} missing in Excel file"))?;
    let mut text = String::new();
    entry
        .take(MAX_PART_BYTES)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    Ok(text)
}

/// Path of the first sheet whose name starts with "GVC".
fn gvc_sheet_path(zip: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Result<String, String> {
    let workbook = read_part(zip, "xl/workbook.xml")?;
    let mut rel_id = None;
    let mut reader = Reader::from_str(&workbook);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e) | Event::Start(e)) if e.local_name().as_ref() == b"sheet" => {
                let attr = |n: &[u8]| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.local_name().as_ref() == n)
                        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
                };
                if attr(b"name").is_some_and(|n| n.trim_start().starts_with("GVC")) {
                    rel_id = attr(b"id");
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let rel_id = rel_id.ok_or("no sheet named \"GVC …\" in the Excel file")?;
    let rels = read_part(zip, "xl/_rels/workbook.xml.rels")?;
    let mut reader = Reader::from_str(&rels);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e) | Event::Start(e)) if e.local_name().as_ref() == b"Relationship" => {
                let attr = |n: &[u8]| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.local_name().as_ref() == n)
                        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
                };
                if attr(b"Id").as_deref() == Some(rel_id.as_str()) {
                    let target = attr(b"Target").ok_or("sheet target missing")?;
                    let target = target.trim_start_matches('/');
                    return Ok(if target.starts_with("xl/") {
                        target.to_string()
                    } else {
                        format!("xl/{target}")
                    });
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    Err("GVC sheet not found in the Excel file".into())
}

/// The shared string table: one entry per `<si>`, rich-text runs joined.
fn shared_strings(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut current: Option<String> = None;
    let mut in_t = false;
    let mut in_rph = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"si" => current = Some(String::new()),
                b"t" => in_t = true,
                b"rPh" => in_rph = true, // phonetic hints are not part of the text
                _ => {}
            },
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"si" => out.push(String::new()),
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"si" => out.extend(current.take()),
                b"t" => in_t = false,
                b"rPh" => in_rph = false,
                _ => {}
            },
            Ok(Event::Text(t)) if in_t && !in_rph => {
                if let (Some(s), Ok(v)) = (current.as_mut(), t.unescape()) {
                    s.push_str(&v);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// Zero-based column index of a cell reference like "C12".
fn column(reference: &str) -> usize {
    reference
        .bytes()
        .take_while(u8::is_ascii_alphabetic)
        .fold(0usize, |n, b| {
            n * 26 + usize::from(b.to_ascii_uppercase() - b'A' + 1)
        })
        .saturating_sub(1)
}

/// Cell texts per row (missing cells are empty strings).
fn sheet_rows(xml: &str, strings: &[String]) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: HashMap<usize, String> = HashMap::new();
    let mut reader = Reader::from_str(xml);
    let (mut col, mut kind, mut value, mut in_value) =
        (0usize, String::new(), String::new(), false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"row" => row.clear(),
                b"c" => {
                    let attr = |n: &[u8]| {
                        e.attributes()
                            .flatten()
                            .find(|a| a.key.local_name().as_ref() == n)
                            .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
                    };
                    col = attr(b"r").map(|r| column(&r)).unwrap_or(row.len());
                    kind = attr(b"t").unwrap_or_default();
                    value.clear();
                }
                b"v" | b"t" => in_value = true,
                _ => {}
            },
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"v" | b"t" => in_value = false,
                b"c" => {
                    let text = if kind == "s" {
                        value
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|i| strings.get(i).cloned())
                            .unwrap_or_default()
                    } else {
                        value.clone()
                    };
                    row.insert(col, text);
                }
                b"row" => {
                    let width = row.keys().max().map_or(0, |m| m + 1);
                    rows.push(
                        (0..width)
                            .map(|i| row.remove(&i).unwrap_or_default())
                            .collect(),
                    );
                }
                _ => {}
            },
            Ok(Event::Text(t)) if in_value => {
                if let Ok(v) = t.unescape() {
                    value.push_str(&v);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    rows
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// A minimal .xlsx with an overview sheet and a "GVC ► ISO BTC" sheet.
    /// `rows` are (GVC, name, D/C, domain, family, sub-family).
    pub(crate) fn workbook(rows: &[(&str, &str, &str, &str, &str, &str)]) -> Vec<u8> {
        let mut strings: Vec<String> = vec!["GVC".into(), "Geschäftsvorfallbezeichnung".into()];
        let idx = |s: &str, strings: &mut Vec<String>| {
            strings.iter().position(|x| x == s).unwrap_or_else(|| {
                strings.push(s.to_string());
                strings.len() - 1
            })
        };
        let mut sheet = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>0XX  und 100</t></is></c></row>"#,
        );
        for (i, (gvc, name, dc, d, f, s)) in rows.iter().enumerate() {
            let r = i + 3;
            let n = idx(name, &mut strings);
            let mut cells =
                format!(r#"<c r="A{r}"><v>{gvc}</v></c><c r="B{r}" t="s"><v>{n}</v></c>"#);
            for (c, v) in [("D", dc), ("E", d), ("F", f), ("G", s)] {
                if !v.is_empty() {
                    let k = idx(v, &mut strings);
                    cells += &format!(r#"<c r="{c}{r}" t="s"><v>{k}</v></c>"#);
                }
            }
            sheet += &format!(r#"<row r="{r}">{cells}</row>"#);
        }
        sheet += "</sheetData></worksheet>";
        let sst: String = strings
            .iter()
            .map(|s| format!("<si><t>{s}</t></si>"))
            .collect();
        let parts = [
            ("xl/workbook.xml", r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="DE-Überblick" sheetId="1" r:id="rId1"/><sheet name="GVC ► ISO BTC" sheetId="3" r:id="rId3"/></sheets></workbook>"#.to_string()),
            ("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId3" Type="worksheet" Target="worksheets/sheet3.xml"/></Relationships>"#.to_string()),
            ("xl/sharedStrings.xml", format!(r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">{sst}</sst>"#)),
            ("xl/worksheets/sheet1.xml", r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#.to_string()),
            ("xl/worksheets/sheet3.xml", sheet),
        ];
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in parts {
                zw.start_file(name, opts).unwrap();
                zw.write_all(body.as_bytes()).unwrap();
            }
            zw.finish().unwrap();
        }
        buf.into_inner()
    }

    fn sample() -> GvcTable {
        parse_workbook(
            &workbook(&[
                (
                    "166",
                    "SEPA Credit Transfer (Einzelbuchung-Haben)",
                    "C",
                    "PMNT",
                    "RCDT",
                    "ESCT",
                ),
                ("201", "Zahlungsauftrag", "D", "PMNT", "ICDT", "XBCT"),
                (
                    "201",
                    "Rückgabe Zahlungseingang",
                    "D",
                    "PMNT",
                    "RCDT",
                    "XRTN",
                ),
                ("6", "Kreditkartenabrechnung", "D", "PMNT", "CCRD", "POSC"),
            ]),
            "Anhang1.xlsx",
        )
        .unwrap()
    }

    #[test]
    fn reads_the_gvc_sheet_and_skips_header_and_section_rows() {
        let t = sample();
        assert_eq!(t.source, "Anhang1.xlsx");
        assert_eq!(t.rows.len(), 4);
        assert_eq!(t.codes(), 3);
        assert_eq!(
            t.rows[0],
            GvcRow {
                gvc: "166".into(),
                name: "SEPA Credit Transfer (Einzelbuchung-Haben)".into(),
                iso: "PMNT-RCDT-ESCT".into(),
                side: "C".into(),
            }
        );
        assert_eq!(t.rows[3].gvc, "006", "short codes are padded like in camt");
    }

    #[test]
    fn names_prefer_the_row_matching_iso_code_and_side() {
        let t = sample();
        assert_eq!(
            t.name("166", None, None),
            Some("SEPA Credit Transfer (Einzelbuchung-Haben)")
        );
        assert_eq!(
            t.name("201", Some("PMNT-RCDT-XRTN"), Some("DBIT")),
            Some("Rückgabe Zahlungseingang")
        );
        assert_eq!(
            t.name("201", Some("PMNT-ICDT-XBCT"), Some("DBIT")),
            Some("Zahlungsauftrag")
        );
        assert_eq!(t.name("201", None, None), Some("Zahlungsauftrag"));
        assert_eq!(t.name("999", None, None), None);
    }

    #[test]
    fn a_workbook_without_gvc_sheet_is_rejected() {
        let bytes = workbook(&[]);
        assert!(parse_workbook(&bytes, "x.xlsx").is_err());
        assert!(parse_workbook(b"not a zip", "x.xlsx").is_err());
    }

    #[test]
    fn the_table_round_trips_through_the_schema_dir() {
        let dir = std::env::temp_dir().join("sepa_gvc_roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let t = sample();
        t.save(&dir).unwrap();
        assert_eq!(GvcTable::load(&dir), Some(t));
    }

    #[test]
    fn cell_references_map_to_columns() {
        assert_eq!(column("A1"), 0);
        assert_eq!(column("G12"), 6);
        assert_eq!(column("AA3"), 26);
    }
}
