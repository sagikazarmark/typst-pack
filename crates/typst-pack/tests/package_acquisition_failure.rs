use typst::syntax::package::{PackageSpec, PackageVersion};
use typst_pack::{PackageAcquisitionFailure, PackageAcquisitionFailureReason};

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
