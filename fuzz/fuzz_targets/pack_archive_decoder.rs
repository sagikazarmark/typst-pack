#![no_main]

use std::collections::BTreeSet;
use std::io::{Cursor, Read, Write};
use std::sync::LazyLock;

use libfuzzer_sys::fuzz_target;
use typst_pack::Pack;
use typst_pack::archive::{DecodeLimits, decode, encode};

/// A valid pack with project files, an embedded package, and an embedded
/// font container: the seed the structured mode mutates.
static SEED: LazyLock<Vec<(String, Vec<u8>)>> = LazyLock::new(|| {
    let font = typst_pack::font::embedded().next().unwrap();
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"#import \"@local/lib:1.0.0\": x\n#x".to_vec())
        .file("data/table.csv", b"a,b\n".to_vec())
        .package_file(
            "@local/lib:1.0.0".parse().unwrap(),
            "typst.toml",
            b"[package]\nname = \"lib\"\nversion = \"1.0.0\"\nentrypoint = \"lib.typ\"\n".to_vec(),
        )
        .package_file(
            "@local/lib:1.0.0".parse().unwrap(),
            "lib.typ",
            b"#let x = 1".to_vec(),
        )
        .font(font.data().to_vec())
        .build()
        .unwrap();
    members(&encode(&pack))
});

fuzz_target!(|data: &[u8]| {
    check(data);

    // Structured mode: rename one seed member or add an alias of one, with a
    // name taken from the input. No such variant may be read as anything but
    // the seed.
    let [operation, index, name @ ..] = data else {
        return;
    };
    let mut members = SEED.clone();
    let index = usize::from(*index) % members.len();
    let name = String::from_utf8_lossy(name).into_owned();
    if operation % 2 == 0 {
        members[index].0 = name;
    } else {
        let data = members[index].1.clone();
        members.push((name, data));
    }
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in members {
        if writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .is_err()
        {
            return;
        }
        writer.write_all(&data).unwrap();
    }
    check(&writer.finish().unwrap().into_inner());
});

/// Every accepted archive holds exactly the members its re-encoding holds,
/// and re-encodes into an equal pack.
fn check(data: &[u8]) {
    let Ok(pack) = decode(data, DecodeLimits::default()) else {
        return;
    };
    let archive = encode(&pack);
    let reread = decode(&archive, DecodeLimits::default()).expect("re-encoding is decodable");
    assert_eq!(reread.identity(), pack.identity());
    let names = |archive: &[u8]| {
        members(archive)
            .into_iter()
            .map(|(name, _)| name)
            .filter(|name| !name.ends_with('/'))
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        names(data),
        names(&archive),
        "an accepted member name is not canonical"
    );
}

fn members(archive: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).unwrap();
    (0..zip.len())
        .map(|index| {
            let mut entry = zip.by_index(index).unwrap();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            (entry.name().to_owned(), data)
        })
        .collect()
}
