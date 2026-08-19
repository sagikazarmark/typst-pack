#![no_main]

use std::num::NonZeroUsize;

use libfuzzer_sys::fuzz_target;
use typst::foundations::{Dict, Value};
use typst_pack::{
    CompilationFulfillmentSet, CompilationOutputSpecification, CompilationRequestIssue,
    DocumentTime, HtmlOutputSpecification, Pack, PackCompilationRequest, PackOverrideSet,
    PackageTree, PackageTreeFulfillment, PngOutputSpecification, RequestValueOrigin,
    SvgOutputSpecification, compile_with_limits,
};

fuzz_target!(|data: &[u8]| {
    let split = data.len() / 2;
    let first = &data[..split];
    let second = &data[split..];
    let reverse = data.first().is_some_and(|byte| byte & 1 != 0);
    let flags = data.get(1).copied().unwrap_or_default();
    let invalid_ppi = flags & 1 != 0;
    let mismatched_overrides = flags & 2 != 0;
    let invalid_document_time = flags & 4 != 0;
    let pack = Pack::builder("main.typ")
        .file("main.typ", b"request fuzz".to_vec())
        .unwrap()
        .file("a.txt", b"a".to_vec())
        .unwrap()
        .file("b.txt", b"b".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let other_pack = Pack::builder("main.typ")
        .file("main.typ", b"other request fuzz".to_vec())
        .unwrap()
        .file("a.txt", b"other a".to_vec())
        .unwrap()
        .file("b.txt", b"other b".to_vec())
        .unwrap()
        .build()
        .unwrap();
    let build_overrides = |owner: &Pack, reverse: bool| {
        let entries = if reverse {
            [("b.txt", second), ("a.txt", first)]
        } else {
            [("a.txt", first), ("b.txt", second)]
        };
        entries
            .into_iter()
            .fold(PackOverrideSet::new(owner), |overrides, (path, bytes)| {
                overrides.replace(path, bytes.to_vec()).unwrap()
            })
    };
    let caller_inputs = {
        let mut inputs = Dict::new();
        inputs.insert(
            "caller".into(),
            Value::Str(String::from_utf8_lossy(data).into_owned().into()),
        );
        inputs
    };
    let adapter_inputs = {
        let mut inputs = Dict::new();
        inputs.insert("adapter".into(), Value::Int(data.len() as i64));
        inputs
    };
    if flags & 8 != 0 {
        let report = compile_with_limits(
            PackCompilationRequest::new(
                pack.clone(),
                CompilationOutputSpecification::Svg(SvgOutputSpecification {
                    pretty: reverse,
                    ..SvgOutputSpecification::default()
                }),
            )
            .inputs(caller_inputs.clone()),
            typst_pack::CompilationLimits::reference_v1(),
        )
        .unwrap();
        assert!(report.result().is_some());
    }
    let invalid_output = CompilationOutputSpecification::Png(PngOutputSpecification {
        page_selection: typst_pack::PageSelection::new(vec![
            Some(NonZeroUsize::new(2).unwrap())..=Some(NonZeroUsize::new(1).unwrap()),
        ]),
        pixels_per_inch: Some(if invalid_ppi { f64::NAN } else { 144.0 }),
        render_bleed: false,
    });
    let adapter_output =
        CompilationOutputSpecification::Html(HtmlOutputSpecification { pretty: reverse });
    let undeclared: typst::syntax::package::PackageSpec =
        "@local/undeclared:1.0.0".parse().unwrap();
    let fulfillments = || {
        CompilationFulfillmentSet::new(
            [PackageTreeFulfillment::new(
                undeclared.clone(),
                PackageTree::from_owned_entries([("lib.typ", data.to_vec())]).unwrap(),
            )],
            [],
        )
        .unwrap()
    };
    let feature_operations = data
        .iter()
        .map(|byte| match byte % 6 {
            0 => (typst::Feature::Html, RequestValueOrigin::AdapterResolved),
            1 => (typst::Feature::Html, RequestValueOrigin::CallerSupplied),
            2 => (typst::Feature::Bundle, RequestValueOrigin::AdapterResolved),
            3 => (typst::Feature::Bundle, RequestValueOrigin::CallerSupplied),
            4 => (
                typst::Feature::A11yExtras,
                RequestValueOrigin::AdapterResolved,
            ),
            _ => (
                typst::Feature::A11yExtras,
                RequestValueOrigin::CallerSupplied,
            ),
        })
        .collect::<Vec<_>>();
    let expected_features = [
        typst::Feature::Html,
        typst::Feature::Bundle,
        typst::Feature::A11yExtras,
    ]
    .into_iter()
    .filter_map(|feature| {
        let origins = feature_operations
            .iter()
            .filter_map(|(value, origin)| (*value == feature).then_some(*origin));
        if origins
            .clone()
            .any(|origin| origin == RequestValueOrigin::CallerSupplied)
        {
            Some((feature, RequestValueOrigin::CallerSupplied))
        } else if origins
            .clone()
            .any(|origin| origin == RequestValueOrigin::AdapterResolved)
        {
            Some((feature, RequestValueOrigin::AdapterResolved))
        } else {
            None
        }
    })
    .collect::<Vec<_>>();
    let build_request = |reverse_origins: bool, reverse_overrides: bool| {
        let mut request = PackCompilationRequest::new_with_adapter_resolved_output(
            pack.clone(),
            CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
        );
        if reverse_origins {
            request = request
                .inputs(caller_inputs.clone())
                .adapter_resolved_inputs(adapter_inputs.clone())
                .output(invalid_output.clone())
                .adapter_resolved_output(adapter_output.clone())
                .document_time(DocumentTime::UnixTimestamp(if invalid_document_time {
                    i64::MAX
                } else {
                    0
                }))
                .adapter_resolved_document_time(DocumentTime::UnixTimestamp(0));
        } else {
            request = request
                .adapter_resolved_inputs(adapter_inputs.clone())
                .inputs(caller_inputs.clone())
                .adapter_resolved_output(adapter_output.clone())
                .output(invalid_output.clone())
                .adapter_resolved_document_time(DocumentTime::UnixTimestamp(0))
                .document_time(DocumentTime::UnixTimestamp(if invalid_document_time {
                    i64::MAX
                } else {
                    0
                }));
        }
        let override_pack = if mismatched_overrides {
            &other_pack
        } else {
            &pack
        };
        request = request
            .overrides(build_overrides(override_pack, reverse_overrides))
            .adapter_resolved_overrides(PackOverrideSet::new(&pack));
        let operations = if reverse_origins {
            feature_operations.iter().rev().copied().collect::<Vec<_>>()
        } else {
            feature_operations.clone()
        };
        for (feature, origin) in operations {
            request = match origin {
                RequestValueOrigin::CallerSupplied => request.feature(feature),
                RequestValueOrigin::AdapterResolved => request.adapter_resolved_feature(feature),
                _ => unreachable!(),
            };
        }
        request.fulfillments(fulfillments())
    };

    let first_rejection = compile_with_limits(
        build_request(reverse, reverse),
        typst_pack::CompilationLimits::reference_v1(),
    )
    .unwrap_err();
    let second_rejection = compile_with_limits(
        build_request(!reverse, !reverse),
        typst_pack::CompilationLimits::reference_v1(),
    )
    .unwrap_err();
    for rejection in [&first_rejection, &second_rejection] {
        let issue_kinds = rejection
            .issues()
            .iter()
            .map(|issue| match issue {
                CompilationRequestIssue::InvalidPageRange { start, end }
                    if start.get() == 2 && end.get() == 1 =>
                {
                    0
                }
                CompilationRequestIssue::InvalidPpi => 1,
                CompilationRequestIssue::OverrideSetPackMismatch => 2,
                CompilationRequestIssue::UnsupportedBundleFeature => 3,
                CompilationRequestIssue::InvalidDocumentTimestamp => 4,
                issue => panic!("unexpected request issue: {issue:?}"),
            })
            .collect::<Vec<_>>();
        let mut expected_issues = vec![0];
        if invalid_ppi {
            expected_issues.push(1);
        }
        if mismatched_overrides {
            expected_issues.push(2);
        }
        if expected_features
            .iter()
            .any(|(feature, _)| *feature == typst::Feature::Bundle)
        {
            expected_issues.push(3);
        }
        if invalid_document_time {
            expected_issues.push(4);
        }
        assert_eq!(issue_kinds, expected_issues);
        let inventory = rejection.request_inventory();
        assert_eq!(
            inventory.output_specification().origin(),
            RequestValueOrigin::CallerSupplied
        );
        assert_eq!(
            inventory.inputs().origin(),
            RequestValueOrigin::CallerSupplied
        );
        assert_eq!(
            inventory.overrides().origin(),
            RequestValueOrigin::CallerSupplied
        );
        assert_eq!(
            inventory.document_time().origin(),
            RequestValueOrigin::CallerSupplied
        );
        assert_eq!(
            inventory
                .features()
                .iter()
                .map(|feature| (feature.value(), feature.origin()))
                .collect::<Vec<_>>(),
            expected_features
        );
    }
    assert_eq!(
        first_rejection.request_inventory().inputs().value(),
        second_rejection.request_inventory().inputs().value()
    );
    let overrides = |rejection: &typst_pack::CompilationRequestRejection| {
        rejection
            .request_inventory()
            .overrides()
            .value()
            .iter()
            .map(|entry| {
                (
                    entry.path().to_owned(),
                    entry.byte_len(),
                    entry.commitment(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(overrides(&first_rejection), overrides(&second_rejection));
});
