#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::{
    FilesystemWriteError, Pack, PackExtractionSelection, WritePolicy, plan_pack_extraction,
    write_pack_extraction_plan_to_filesystem,
};

fuzz_target!(|data: &[u8]| {
    let flags = data.first().copied().unwrap_or_default();
    let mut builder = Pack::builder("main.typ")
        .file("main.typ", data.to_vec())
        .unwrap();
    for index in 0..3 {
        let payload = data
            .iter()
            .skip(index + 1)
            .step_by(3)
            .copied()
            .collect::<Vec<_>>();
        builder = builder.file(format!("files/{index}.bin"), payload).unwrap();
    }
    let pack = builder.build().unwrap();
    let plan = plan_pack_extraction(&pack, PackExtractionSelection::default()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let destination = std::fs::canonicalize(directory.path())
        .unwrap()
        .join("written");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("unrelated.txt"), b"unrelated").unwrap();
    if flags & 1 != 0 {
        std::fs::write(destination.join("main.typ"), b"existing").unwrap();
    }
    if flags & 2 != 0 {
        std::fs::write(destination.join("files"), b"a file, not a directory").unwrap();
    }

    for byte in data.iter().copied().skip(1).take(8) {
        let policy = if byte & 1 == 0 {
            WritePolicy::CreateNew
        } else {
            WritePolicy::Overwrite
        };
        let before = snapshot(&destination);
        match write_pack_extraction_plan_to_filesystem(&plan, &destination, policy) {
            Ok(()) => {
                for entry in plan.entries() {
                    assert_eq!(
                        std::fs::read(destination.join(entry.relative_path())).unwrap(),
                        entry.bytes()
                    );
                }
            }
            // A refused write changes nothing.
            Err(FilesystemWriteError::Rejected { .. }) => {
                assert_eq!(snapshot(&destination), before);
            }
            Err(error) => panic!("unexpected write failure: {error}"),
        }
        // No temporary file survives any outcome.
        assert!(
            snapshot(&destination)
                .iter()
                .all(|(name, _)| !name.contains(".typst-pack-"))
        );
    }
    assert_eq!(
        std::fs::read(destination.join("unrelated.txt")).unwrap(),
        b"unrelated"
    );
});

/// Every file beneath `root` with its bytes, sorted by path.
fn snapshot(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let name = path.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                files.push((name, std::fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}
