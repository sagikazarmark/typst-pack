#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::{
    FilesystemMergePolicy, FilesystemPublicationFaultProbe, Pack, PackExtractionSelection,
    plan_pack_extraction, publish_pack_extraction_plan_to_filesystem_with_fault_probe,
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
        .join("published");
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("unrelated.txt"), b"unrelated").unwrap();
    if flags & 1 != 0 {
        std::fs::write(destination.join("main.typ"), b"existing").unwrap();
    }
    let planned = plan
        .entries()
        .iter()
        .map(|entry| std::path::PathBuf::from(entry.relative_path()))
        .collect::<Vec<_>>();
    for (step, byte) in data.iter().copied().skip(1).take(16).enumerate() {
        let policy = match byte % 3 {
            0 => FilesystemMergePolicy::PublishNewTree,
            1 => FilesystemMergePolicy::MergeCreateOnly,
            _ => FilesystemMergePolicy::MergeReplaceExactFiles,
        };
        let publish_destination = if policy == FilesystemMergePolicy::PublishNewTree {
            destination.with_file_name(format!("new-tree-{step}"))
        } else {
            destination.clone()
        };
        let fault_file =
            usize::from(data.get(step + 2).copied().unwrap_or_default()) % plan.entries().len();
        let fault_after = usize::from(data.get(step + 3).copied().unwrap_or_default());
        let fault_kind = if policy == FilesystemMergePolicy::PublishNewTree {
            byte / 3 % 8
        } else {
            byte / 3 % 6
        };
        let probe = FilesystemPublicationFaultProbe {
            maximum_write: if fault_kind == 1 {
                fault_after % 4 + 1
            } else {
                usize::MAX
            },
            write_fault_file: (fault_kind == 2).then_some(fault_file),
            write_fault_after: fault_after,
            flush_fault_file: (fault_kind == 3).then_some(fault_file),
            commit_fault_file: (policy != FilesystemMergePolicy::PublishNewTree && fault_kind == 4)
                .then_some(fault_file),
            ancestor_symlink_race_file: (policy != FilesystemMergePolicy::PublishNewTree
                && fault_kind == 5)
                .then_some(fault_file),
            new_tree_commit_unsupported: policy == FilesystemMergePolicy::PublishNewTree
                && fault_kind == 4,
            new_tree_policy_unsupported: policy == FilesystemMergePolicy::PublishNewTree
                && fault_kind == 7,
            tree_staging_open_fault: policy == FilesystemMergePolicy::PublishNewTree
                && matches!(fault_kind, 5 | 6),
            tree_staging_cleanup_fault: policy == FilesystemMergePolicy::PublishNewTree
                && fault_kind == 6,
        };

        let result = publish_pack_extraction_plan_to_filesystem_with_fault_probe(
            &plan,
            &publish_destination,
            policy,
            probe,
        );
        let committed = match &result {
            Ok(receipt) => receipt.completed(),
            Err(error) => error.progress().completed(),
        }
        .iter()
        .map(|entry| std::path::PathBuf::from(entry.relative_path()))
        .collect::<Vec<_>>();
        assert_eq!(committed, planned[..committed.len()]);
        for relative_path in &committed {
            let entry = plan
                .entries()
                .iter()
                .find(|entry| std::path::Path::new(entry.relative_path()) == relative_path)
                .unwrap();
            match std::fs::read(publish_destination.join(relative_path)) {
                Ok(bytes) => assert_eq!(bytes, entry.bytes()),
                Err(error) if fault_kind == 5 && error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("committed file cannot be observed: {error}"),
            }
        }
    }
    assert_eq!(
        std::fs::read(destination.join("unrelated.txt")).unwrap(),
        b"unrelated"
    );
});
