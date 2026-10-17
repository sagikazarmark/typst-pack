#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::{PackInvariantIssue, ProjectSnapshot};

fuzz_target!(|data: &[u8]| {
    let stem: String = data
        .iter()
        .take(16)
        .map(|byte| char::from(b'a' + byte % 26))
        .collect();
    let stem = if stem.is_empty() { "empty" } else { &stem };
    let canonical = format!("files/{stem}.typ");
    let alias = format!("./{canonical}");

    let canonicalized = ProjectSnapshot::new(&alias, [(alias.as_str(), data.to_vec())])
        .unwrap();
    assert_eq!(canonicalized.entrypoint(), canonical);
    assert_eq!(canonicalized.file(&canonical), Some(data));

    let forward = ProjectSnapshot::new("main.typ", [
            ("main.typ", b"main".to_vec()),
            (canonical.as_str(), data.to_vec()),
        ])
        .unwrap();
    let reverse = ProjectSnapshot::new("main.typ", [
            (canonical.as_str(), data.to_vec()),
            ("main.typ", b"main".to_vec()),
        ])
        .unwrap();
    assert_eq!(forward, reverse);

    let duplicate = ProjectSnapshot::new("main.typ", [
            ("main.typ", b"main".to_vec()),
            (canonical.as_str(), b"first".to_vec()),
            (alias.as_str(), b"second".to_vec()),
        ])
        .unwrap_err();
    assert_eq!(
        duplicate.issues(),
        [PackInvariantIssue::DuplicateProjectPath {
            path: canonical.clone(),
        }]
    );

    let missing = ProjectSnapshot::new("main.typ", [(canonical.as_str(), data.to_vec())])
        .unwrap_err();
    assert_eq!(
        missing.issues(),
        [PackInvariantIssue::MissingEntrypoint {
            path: "main.typ".to_owned(),
        }]
    );

    let pack_path = format!("archives/{stem}.typk/inside.typ");
    let excluded = ProjectSnapshot::new("main.typ", [
            ("main.typ", b"main".to_vec()),
            (pack_path.as_str(), data.to_vec()),
        ])
        .unwrap_err();
    assert!(
        matches!(excluded.issues(), [PackInvariantIssue::InvalidPath { path, .. }] if path == &pack_path)
    );

    let mut fields = data.split(|byte| *byte == 0);
    let entrypoint = String::from_utf8_lossy(fields.next().unwrap_or_default()).into_owned();
    let mut entries = Vec::new();
    while let Some(path) = fields.next() {
        let bytes = fields.next().unwrap_or_default();
        entries.push((String::from_utf8_lossy(path).into_owned(), bytes.to_vec()));
    }
    let _ = ProjectSnapshot::new(entrypoint, entries);
});
