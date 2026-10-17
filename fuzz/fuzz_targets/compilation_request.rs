#![no_main]

use std::num::NonZeroUsize;

use libfuzzer_sys::fuzz_target;
use typst::syntax::package::PackageSpec;
use typst_pack::compile::{
    DocumentTime, Input, Output, OutputFormat, PageSelection, RequestIssue,
};
use typst_pack::{Pack, compile, package};

// Request validation reports exactly the invalid values, whatever the input
// order, and caller inputs never make a valid request fail validation.
fuzz_target!(|data: &[u8]| {
    let reverse = data.first().is_some_and(|byte| byte & 1 != 0);
    let flags = data.get(1).copied().unwrap_or_default();
    let invalid_ppi = flags & 1 != 0;
    let invalid_document_time = flags & 4 != 0;
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"request fuzz".to_vec())
        .build()
        .unwrap();
    let mut inputs = vec![
        ("text".to_owned(), Input::Str(String::from_utf8_lossy(data).into_owned())),
        ("bytes".to_owned(), Input::from(data.to_vec())),
    ];
    if reverse {
        inputs.reverse();
    }
    let undeclared: PackageSpec = "@local/undeclared:1.0.0".parse().unwrap();
    let unrequired = package::Tree::new([("lib.typ", data.to_vec())]).unwrap();

    if flags & 8 != 0 {
        let mut request = compile::Request::new(
            pack.clone(),
            Output {
                pretty: reverse, ..Output::new(OutputFormat::Svg) },
        );
        request.options.inputs = inputs.clone();
        // Dependencies the pack does not require are ignored.
        request.packages = vec![(undeclared.clone(), unrequired.clone())];
        assert!(compile(&request).is_ok());
    }

    let features = data
        .iter()
        .map(|byte| match byte % 3 {
            0 => typst::Feature::Html,
            1 => typst::Feature::Bundle,
            _ => typst::Feature::A11yExtras,
        })
        .collect::<Vec<_>>();
    let mut request = compile::Request::new(
        pack,
        Output {
            pages: PageSelection { ranges: vec![
                Some(NonZeroUsize::new(2).unwrap())..=Some(NonZeroUsize::new(1).unwrap()),
            ] },
            ppi: Some(if invalid_ppi { f64::NAN } else { 144.0 }),
            render_bleed: false, ..Output::new(OutputFormat::Png) },
    );
    request.options = compile::Options {
        inputs,
        document_time: DocumentTime::UnixTimestamp(if invalid_document_time { i64::MAX } else { 0 }),
        features: features.clone(),
    };
    request.packages = vec![(undeclared, unrequired)];

    let Err(compile::Error::InvalidRequest(issues)) = compile(&request) else {
        panic!("an invalid request was compiled");
    };
    let mut kinds = issues
        .iter()
        .map(|issue| match issue {
            RequestIssue::InvalidPageRange { start, end } if start.get() == 2 && end.get() == 1 => 0,
            RequestIssue::InvalidPpi => 1,
            RequestIssue::UnsupportedBundleFeature => 2,
            RequestIssue::InvalidDocumentTimestamp => 3,
            issue => panic!("unexpected request issue: {issue:?}"),
        })
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    let mut expected = vec![0];
    if invalid_ppi {
        expected.push(1);
    }
    if features.contains(&typst::Feature::Bundle) {
        expected.push(2);
    }
    if invalid_document_time {
        expected.push(3);
    }
    assert_eq!(kinds, expected);
});
