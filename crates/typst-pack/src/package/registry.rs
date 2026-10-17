//! Reading packages from the official Typst Universe registry with a
//! transport of the caller's choice.
//!
//! Creation reports the exact specifications it needs and was not given.
//! [`archive_url`] names where the registry serves each one, and
//! `expand_archive` (with the `package-reading` feature) turns the fetched
//! bytes into a [`Tree`](super::Tree). Fetching is the
//! caller's: nothing here needs an HTTP client, so a host whose only network
//! access is asynchronous can use these helpers.

use typst::syntax::package::PackageSpec;

#[cfg(feature = "package-reading")]
pub use super::expand::{ExpandError, expand_archive};

/// The URL of the official Typst Universe registry.
const URL: &str = "https://packages.typst.org";

/// The namespace the official registry serves.
const NAMESPACE: &str = "preview";

/// The URL of the archive the registry serves for `spec`, or `None` when the
/// registry does not serve its namespace (only `preview` is served).
///
/// A Typst import always names an exact version, so no index lookup is
/// involved.
pub fn archive_url(spec: &PackageSpec) -> Option<String> {
    let key = archive_key(spec)?;
    Some(format!("{URL}/{key}"))
}

/// Where the registry serves one specification's archive, relative to its
/// root, or `None` when it does not serve the specification's namespace.
pub(crate) fn archive_key(spec: &PackageSpec) -> Option<String> {
    (spec.namespace == NAMESPACE)
        .then(|| format!("{}/{}-{}.tar.gz", spec.namespace, spec.name, spec.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_preview_namespace_has_a_registry_url() {
        let served = "@preview/example:1.2.3".parse().unwrap();
        assert_eq!(
            archive_url(&served).as_deref(),
            Some("https://packages.typst.org/preview/example-1.2.3.tar.gz")
        );
        for unserved in ["@local/example:1.0.0", "@custom/example:1.0.0"] {
            assert_eq!(archive_url(&unserved.parse().unwrap()), None, "{unserved}");
        }
        assert_eq!(
            archive_key(&"@preview/nested-name:10.0.1".parse().unwrap()).as_deref(),
            Some("preview/nested-name-10.0.1.tar.gz")
        );
    }
}
