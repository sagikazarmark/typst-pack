#![no_main]

use std::num::NonZeroUsize;

use libfuzzer_sys::fuzz_target;
use typst::syntax::package::PackageSpec;
use typst_pack::{
    CompileError, CompileOptions, CompileRequest, DocumentTime, Input, Output, Pack, PackageTree,
    PageSelection, PngOutput, RequestIssue, SvgOutput, compile,
};

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
        ("bytes".to_owned(), Input::Bytes(data.to_vec())),
    ];
    if reverse {
        inputs.reverse();
    }
    let undeclared: PackageSpec = "@local/undeclared:1.0.0".parse().unwrap();
    let unrequired = PackageTree::from_owned_entries([("lib.typ", data.to_vec())]).unwrap();

    if flags & 8 != 0 {
        let mut request = CompileRequest::new(
            pack.clone(),
            Output::Svg(SvgOutput {
                pretty: reverse,
                ..SvgOutput::default()
            }),
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
    let mut request = CompileRequest::new(
        pack,
        Output::Png(PngOutput {
            page_selection: PageSelection::new(vec![
                Some(NonZeroUsize::new(2).unwrap())..=Some(NonZeroUsize::new(1).unwrap()),
            ]),
            pixels_per_inch: Some(if invalid_ppi { f64::NAN } else { 144.0 }),
            render_bleed: false,
        }),
    );
    request.options = CompileOptions {
        inputs,
        document_time: DocumentTime::UnixTimestamp(if invalid_document_time { i64::MAX } else { 0 }),
        features: features.clone(),
    };
    request.packages = vec![(undeclared, unrequired)];

    let Err(CompileError::InvalidRequest(issues)) = compile(&request) else {
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
