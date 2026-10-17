#![no_main]

use std::future::{Future, IntoFuture};
use std::sync::OnceLock;
use std::task::{Context, Poll, Waker};

use libfuzzer_sys::fuzz_target;
use typst_pack::opendal::{Error, Operator, WritePolicy, write_compilation_artifacts};
use typst_pack::{
    CompilationLimits, CompilationOutputSpecification, CompilationResult, Pack,
    PackCompilationRequest, SvgOutputSpecification, compile_with_limits,
};

/// Polls a Memory-service future, which never pends.
fn ready<F: IntoFuture>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future.into_future());
    match future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the Memory service never pends"),
    }
}

// Accepted artifact keys must land at exactly `prefix + key`: OpenDAL's path
// normalization must never alias two accepted keys or move one elsewhere.
fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let result = two_page_result();
    let keys = input.split('\0').take(2).collect::<Vec<_>>();
    if keys.len() != result.artifacts().len() {
        return;
    }
    let policy = if data.first().is_some_and(|byte| byte & 1 == 0) {
        WritePolicy::CreateOrVerify
    } else {
        WritePolicy::Overwrite
    };
    let op = Operator::new(opendal::services::Memory::default()).unwrap();

    match ready(write_compilation_artifacts(&op, "artifacts/", result, &keys, policy)) {
        Ok(written) => {
            assert_eq!(written.len(), keys.len());
            for (key, artifact) in keys.iter().zip(result.artifacts()) {
                let stored = ready(op.read(&format!("artifacts/{key}"))).unwrap();
                assert_eq!(stored.to_vec(), artifact.bytes());
            }
            let listed = ready(op.list_with("").recursive(true))
                .unwrap()
                .into_iter()
                .filter(|entry| entry.metadata().is_file())
                .count();
            assert_eq!(listed, keys.len());
        }
        Err(error) => assert!(
            matches!(error.error, Error::InvalidPath { .. }),
            "unexpected {error:?}"
        ),
    }
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
        compile_with_limits(
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
