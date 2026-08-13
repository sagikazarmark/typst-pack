#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use typst_pack::opendal::publication::{
    CompilationArtifactPublicationRequest, PublicationPolicy,
};
use typst_pack::opendal::{Location, OperatorBinding};
use typst_pack::{
    CompilationLimits, CompilationOutputSpecification, CompilationResult, Pack,
    PackCompilationRequest, SvgOutputSpecification, compile,
};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let result = two_page_result();
    let keys = input
        .split('\0')
        .take(4)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let destination = if data.first().is_some_and(|byte| byte & 1 == 0) {
        Location::parse("fuzz:/artifacts/").unwrap()
    } else {
        Location::parse("fuzz:/artifacts").unwrap()
    };
    let policy = if data.get(1).is_some_and(|byte| byte & 1 == 0) {
        PublicationPolicy::CreateOrVerify
    } else {
        PublicationPolicy::OverwriteExactKeys
    };

    if let Ok(request) =
        CompilationArtifactPublicationRequest::new(result, destination, keys.clone(), policy)
    {
        assert_eq!(request.compilation_result_identity(), result.result_identity());
        assert_eq!(request.artifact_keys(), keys);
        assert_eq!(request.policy(), policy);
        for key in request.artifact_keys() {
            let composed = format!("{}{}", request.destination().operation_path(), key);
            assert!(
                Location::from_operation_path(OperatorBinding::new("fuzz").unwrap(), composed)
                    .is_ok()
            );
        }
    }

    let literal_percent = CompilationArtifactPublicationRequest::new(
        result,
        Location::parse("fuzz:/artifacts/").unwrap(),
        ["tree%", "tree%/page%2F.svg"],
        policy,
    )
    .unwrap();
    assert_eq!(
        literal_percent.artifact_keys(),
        ["tree%", "tree%/page%2F.svg"]
    );
});

fn two_page_result() -> &'static CompilationResult {
    static RESULT: OnceLock<CompilationResult> = OnceLock::new();
    RESULT.get_or_init(|| {
        let pack = Pack::builder("main.typ")
            .file(
                "main.typ",
                b"#set page(width: 10pt, height: 10pt, margin: 0pt)\n\
                  #rect(width: 1pt, height: 1pt)\n\
                  #pagebreak()\n\
                  #rect(width: 2pt, height: 2pt)"
                    .to_vec(),
            )
            .unwrap()
            .build()
            .unwrap();
        compile(
            PackCompilationRequest::new(
                pack,
                CompilationOutputSpecification::Svg(SvgOutputSpecification::default()),
            ),
            CompilationLimits::reference_v1(),
        )
        .unwrap()
        .result()
        .unwrap()
        .clone()
    })
}
