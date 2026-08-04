#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::{ProjectSnapshotAssembly, ProjectSnapshotIssue};

fuzz_target!(|data: &[u8]| {
    let stem: String = data
        .iter()
        .take(16)
        .map(|byte| char::from(b'a' + byte % 26))
        .collect();
    let stem = if stem.is_empty() { "empty" } else { &stem };
    let canonical = format!("files/{stem}.typ");
    let alias = format!("./{canonical}");

    let canonicalized = ProjectSnapshotAssembly::new(&alias)
        .assemble([(alias.as_str(), data.to_vec())])
        .unwrap();
    assert_eq!(canonicalized.entrypoint(), canonical);
    assert_eq!(canonicalized.file(&canonical), Some(data));

    let forward = ProjectSnapshotAssembly::new("main.typ")
        .assemble([
            ("main.typ", b"main".to_vec()),
            (canonical.as_str(), data.to_vec()),
        ])
        .unwrap();
    let reverse = ProjectSnapshotAssembly::new("main.typ")
        .assemble([
            (canonical.as_str(), data.to_vec()),
            ("main.typ", b"main".to_vec()),
        ])
        .unwrap();
    assert_eq!(forward, reverse);

    let duplicate = ProjectSnapshotAssembly::new("main.typ")
        .assemble([
            ("main.typ", b"main".to_vec()),
            (canonical.as_str(), b"first".to_vec()),
            (alias.as_str(), b"second".to_vec()),
        ])
        .unwrap_err();
    assert_eq!(
        duplicate.issues(),
        [ProjectSnapshotIssue::DuplicatePath {
            path: canonical.clone(),
        }]
    );

    let missing = ProjectSnapshotAssembly::new("main.typ")
        .assemble([(canonical.as_str(), data.to_vec())])
        .unwrap_err();
    assert_eq!(
        missing.issues(),
        [ProjectSnapshotIssue::MissingEntrypoint {
            path: "main.typ".to_owned(),
        }]
    );

    let pack_path = format!("archives/{stem}.typk/inside.typ");
    let excluded = ProjectSnapshotAssembly::new("main.typ")
        .assemble([
            ("main.typ", b"main".to_vec()),
            (pack_path.as_str(), data.to_vec()),
        ])
        .unwrap_err();
    assert!(
        matches!(excluded.issues(), [ProjectSnapshotIssue::InvalidPath { path, .. }] if path == &pack_path)
    );

    let mut fields = data.split(|byte| *byte == 0);
    let entrypoint = String::from_utf8_lossy(fields.next().unwrap_or_default()).into_owned();
    let mut entries = Vec::new();
    while let Some(path) = fields.next() {
        let bytes = fields.next().unwrap_or_default();
        entries.push((String::from_utf8_lossy(path).into_owned(), bytes.to_vec()));
    }
    let _ = ProjectSnapshotAssembly::new(entrypoint).assemble(entries);
});
