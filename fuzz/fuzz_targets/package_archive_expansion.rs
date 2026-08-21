#![no_main]

use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use typst::syntax::package::PackageSpec;
use typst_pack::{PackageExpansionLimits, expand_package_archive};

fuzz_target!(|data: &[u8]| {
    let spec = PackageSpec::from_str("@preview/fuzz:1.0.0").unwrap();
    let limits = PackageExpansionLimits::new(64 * 1024, 128, 8 * 1024, 16 * 1024, 64 * 1024);
    let _ = expand_package_archive(spec.clone(), data, limits);
    let varied_limits = PackageExpansionLimits::new(
        u64::from(data.get(1).copied().unwrap_or_default()) * 256,
        u64::from(data.get(2).copied().unwrap_or_default()),
        u64::from(data.get(3).copied().unwrap_or_default()) * 32,
        u64::from(data.get(4).copied().unwrap_or_default()) * 64,
        u64::from(data.get(5).copied().unwrap_or_default()) * 256,
    );
    let _ = expand_package_archive(spec.clone(), data, varied_limits);

    let mode = data.first().copied().unwrap_or_default() % 8;
    let payload = &data[data.len().min(1)..data.len().min(16 * 1024 + 1)];
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    match mode {
        0 => append(&mut builder, "fuzz.typ", payload),
        1 => append(
            &mut builder,
            &format!("nested/{}.typ", "a".repeat(128)),
            payload,
        ),
        2 => {
            builder
                .append_pax_extensions([("path", payload)])
                .unwrap();
            append(&mut builder, "placeholder", payload);
        }
        3 => {
            let declared = u64::from_le_bytes(
                data.get(..8)
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or([0; 8]),
            )
            .to_string();
            builder
                .append_pax_extensions([("size", declared.as_bytes())])
                .unwrap();
            append(&mut builder, "pax-size", payload);
        }
        4 => {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(payload.len() as u64);
            header.set_mode(0o755);
            builder.append_data(&mut header, "directory", payload).unwrap();
        }
        5 => {
            append(&mut builder, "path", payload);
            append(&mut builder, "path/descendant", payload);
        }
        6 => {
            let mut long_name = tar::Header::new_gnu();
            long_name.set_entry_type(tar::EntryType::GNULongName);
            long_name.set_size(9);
            long_name.set_mode(0o644);
            long_name.set_cksum();
            builder.append(&long_name, &b"gnu.typ\0"[..]).unwrap();
            builder
                .append_pax_extensions([("path", &b"pax.typ"[..])])
                .unwrap();
            append(&mut builder, "placeholder", payload);
        }
        _ => {
            let mut header = tar::Header::new_gnu();
            header.set_size(payload.len() as u64);
            header.set_mode(0o644);
            header.as_gnu_mut().unwrap().name[0] = 0xff;
            header.set_cksum();
            builder.append(&header, payload).unwrap();
        }
    }
    let archive = builder.into_inner().unwrap().finish().unwrap();
    let _ = expand_package_archive(spec.clone(), &archive, limits);
    let _ = expand_package_archive(spec, &archive, varied_limits);
});

fn append(
    builder: &mut tar::Builder<flate2::write::GzEncoder<Vec<u8>>>,
    path: &str,
    data: &[u8],
) {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    builder.append_data(&mut header, path, data).unwrap();
}
