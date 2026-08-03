use typst::syntax::package::{PackageSpec, PackageVersion};
use typst_pack::{
    PackageAcquisitionFailure, PackageAcquisitionFailureReason, PackageAcquisitionFailures,
};

#[test]
fn package_acquisition_failure_preserves_the_exact_specification_and_typed_reason() {
    let spec: PackageSpec = "@preview/example:1.0.0".parse().unwrap();
    let latest: PackageVersion = "1.1.0".parse().unwrap();
    let failure = PackageAcquisitionFailure::new(
        spec.clone(),
        PackageAcquisitionFailureReason::VersionNotFound { latest },
    );

    assert_eq!(failure.spec(), &spec);
    assert_eq!(
        failure.reason(),
        &PackageAcquisitionFailureReason::VersionNotFound { latest }
    );
}

#[test]
fn package_acquisition_failures_are_keyed_in_canonical_specification_order() {
    let alpha: PackageSpec = "@preview/alpha:1.0.0".parse().unwrap();
    let zeta: PackageSpec = "@preview/zeta:1.0.0".parse().unwrap();
    let mut failures = PackageAcquisitionFailures::new();

    failures.insert(PackageAcquisitionFailure::new(
        zeta.clone(),
        PackageAcquisitionFailureReason::NotFound,
    ));
    failures.insert(PackageAcquisitionFailure::new(
        alpha.clone(),
        PackageAcquisitionFailureReason::NetworkFailed { detail: None },
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
        &PackageAcquisitionFailureReason::NetworkFailed { detail: None }
    );
}
