use typst::syntax::package::{PackageSpec, PackageVersion};
use typst_pack::{PackageReadFailure, PackageReadFailureReason, PackageReadFailures};

#[test]
fn package_read_failure_preserves_the_exact_specification_and_typed_reason() {
    let spec: PackageSpec = "@preview/example:1.0.0".parse().unwrap();
    let latest: PackageVersion = "1.1.0".parse().unwrap();
    let failure = PackageReadFailure::new(
        spec.clone(),
        PackageReadFailureReason::VersionNotFound { latest },
    );

    assert_eq!(failure.spec(), &spec);
    assert_eq!(
        failure.reason(),
        &PackageReadFailureReason::VersionNotFound { latest }
    );
}

#[test]
fn package_read_failures_are_keyed_in_canonical_specification_order() {
    let alpha: PackageSpec = "@preview/alpha:1.0.0".parse().unwrap();
    let zeta: PackageSpec = "@preview/zeta:1.0.0".parse().unwrap();
    let mut failures = PackageReadFailures::new();

    failures.insert(PackageReadFailure::new(
        zeta.clone(),
        PackageReadFailureReason::NotFound,
    ));
    failures.insert(PackageReadFailure::new(
        alpha.clone(),
        PackageReadFailureReason::NetworkFailed { detail: None },
    ));

    assert_eq!(
        failures
            .entries()
            .map(|failure| failure.spec().to_string())
            .collect::<Vec<_>>(),
        ["@preview/alpha:1.0.0", "@preview/zeta:1.0.0"]
    );
    assert_eq!(
        failures.get(&alpha).unwrap().reason(),
        &PackageReadFailureReason::NetworkFailed { detail: None }
    );
}
