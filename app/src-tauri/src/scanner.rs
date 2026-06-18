use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Expand a set of input paths (files and/or directories) into a deduplicated,
/// sorted list of `.xml` files. Directories are walked recursively.
/// Skips NTFS `:Zone.Identifier` alternate-stream artifacts.
pub fn expand_paths<I, P>(inputs: I) -> Vec<PathBuf>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut out: Vec<PathBuf> = Vec::new();
    for input in inputs {
        let input = input.as_ref();
        if input.is_dir() {
            for entry in WalkDir::new(input).into_iter().filter_map(Result::ok) {
                if entry.file_type().is_file() && is_xml(entry.path()) {
                    out.push(entry.into_path());
                }
            }
        } else if input.is_file() && is_xml(input) {
            out.push(input.to_path_buf());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn is_xml(p: &Path) -> bool {
    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name.to_ascii_lowercase().ends_with(".xml") && !name.contains(":Zone.Identifier")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn expands_dir_and_filters_non_xml() {
        let dir = std::env::temp_dir().join("sepa_scan_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.xml"), "<a/>").unwrap();
        fs::write(dir.join("b.txt"), "nope").unwrap();
        fs::write(dir.join("sub/c.XML"), "<c/>").unwrap();

        let got = expand_paths([&dir]);
        let names: Vec<_> = got
            .iter()
            .filter_map(|p| p.file_name()?.to_str())
            .map(|s| s.to_string())
            .collect();
        assert!(names.contains(&"a.xml".to_string()));
        assert!(names.contains(&"c.XML".to_string()));
        assert!(!names.iter().any(|n| n.ends_with(".txt")));
    }

    #[test]
    fn passes_through_single_file() {
        let f = std::env::temp_dir().join("sepa_scan_single.xml");
        fs::write(&f, "<a/>").unwrap();
        let got = expand_paths([&f]);
        assert_eq!(got.len(), 1);
    }
}
