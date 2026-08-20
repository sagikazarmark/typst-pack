//! Where the candidates for one package specification live, and which files
//! are font containers.
//!
//! This is the one place typst-pack derives those rules. A source that reaches
//! for a package derives its key here instead of spelling
//! `namespace/name/version` itself, so a tree source, a raw archive cache, and
//! the official registry cannot drift apart, and a second font source selects
//! from the same suffix set as the first instead of restating it.
//!
//! The derivations are the crate's own. `typst-kit` still resolves and stores
//! filesystem package directories under its own equivalent layout, which
//! [`package_tree_key`] must agree with rather than replace.
//!
//! Deriving a key reads, writes, and transports nothing, so this module is
//! ungated: where a candidate lives is the same rule whether or not the build
//! can reach it. The consumer of each rule is feature-gated, so a narrower
//! build compiles rules it never reaches.

use typst::syntax::package::PackageSpec;

/// The URL of the package registry this crate describes the layout of, the
/// official Typst Universe registry. There is no standardized registry
/// protocol, so the layout is this registry's own.
#[cfg_attr(not(feature = "package-reading"), allow(dead_code))]
pub(crate) const PACKAGE_REGISTRY_URL: &str = "https://packages.typst.org";

/// The one package namespace the official registry serves. A specification in
/// any other namespace is resolved from wherever its namespace lives, which the
/// registry layout says nothing about.
#[cfg_attr(not(feature = "package-reading"), allow(dead_code))]
pub(crate) const PACKAGE_REGISTRY_NAMESPACE: &str = "preview";

/// The suffixes a supported Font Container file is named with, without their
/// separating dot. They are matched case-insensitively.
#[cfg_attr(not(feature = "fs"), allow(dead_code))]
pub(crate) const FONT_CONTAINER_EXTENSIONS: [&str; 4] = ["ttf", "ttc", "otf", "otc"];

/// The key of one exact specification's Package Tree, relative to any source
/// holding trees.
///
/// The tree is a prefix rather than one object: every package file lives
/// beneath this key.
#[cfg_attr(not(feature = "egress"), allow(dead_code))]
pub(crate) fn package_tree_key(spec: &PackageSpec) -> String {
    format!("{}/{}/{}", spec.namespace, spec.name, spec.version)
}

/// The key of one exact specification's Package Archive, relative to a cache
/// holding raw archives.
///
/// A cache holds one archive per specification, so its key is the tree layout
/// of that specification naming an archive rather than a prefix.
#[allow(
    dead_code,
    reason = "no shipped adapter reads a raw archive cache yet; the layout is derived here so the first one cannot invent its own"
)]
pub(crate) fn package_archive_cache_key(spec: &PackageSpec) -> String {
    format!("{}.tar.gz", package_tree_key(spec))
}

/// Whether the official registry serves the specification's namespace, and so
/// whether it can have a candidate there at all.
#[cfg_attr(not(feature = "package-reading"), allow(dead_code))]
pub(crate) fn official_registry_serves(spec: &PackageSpec) -> bool {
    spec.namespace == PACKAGE_REGISTRY_NAMESPACE
}

/// The key of one exact specification's Package Archive relative to the
/// official registry, or `None` when the registry does not serve its namespace.
///
/// No index lookup is involved: a Typst import specification always carries an
/// exact version, so an archive is addressed directly.
#[cfg_attr(not(feature = "package-reading"), allow(dead_code))]
pub(crate) fn official_registry_archive_key(spec: &PackageSpec) -> Option<String> {
    official_registry_serves(spec)
        .then(|| format!("{}/{}-{}.tar.gz", spec.namespace, spec.name, spec.version))
}

/// The URL of the archive the official registry serves for one exact
/// specification, or `None` when it does not serve that namespace.
#[cfg_attr(not(feature = "package-reading"), allow(dead_code))]
pub(crate) fn official_registry_archive_url(spec: &PackageSpec) -> Option<String> {
    let key = official_registry_archive_key(spec)?;
    Some(format!("{PACKAGE_REGISTRY_URL}/{key}"))
}

/// Whether a file extension names a supported Font Container.
///
/// This is the rule for a source that addresses a font by path, where a name
/// is an extension only once a stem precedes it: a file named `.ttf` has no
/// extension and is not a container.
#[cfg_attr(not(feature = "fs"), allow(dead_code))]
pub(crate) fn is_font_container_extension(extension: &str) -> bool {
    FONT_CONTAINER_EXTENSIONS
        .iter()
        .any(|supported| extension.eq_ignore_ascii_case(supported))
}

/// Whether a key names a supported Font Container.
///
/// This is the rule for a source that addresses a font by key rather than by
/// path, where the whole key is one string and a supported suffix terminating
/// it is all that is asked. It selects the keys
/// [`is_font_container_extension`] selects paths for, plus a key that is
/// nothing but a suffix, such as `.ttf`.
#[cfg_attr(
    not(feature = "opendal"),
    allow(
        dead_code,
        reason = "no enabled source addresses Font Containers by key"
    )
)]
pub(crate) fn is_font_container_key(key: &str) -> bool {
    key.rsplit_once('.')
        .is_some_and(|(_, suffix)| is_font_container_extension(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(text: &str) -> PackageSpec {
        text.parse().expect("the test specification parses")
    }

    #[test]
    fn every_source_derives_one_specification_from_its_namespace_name_and_version() {
        let layouts = [
            (
                "@preview/example:1.2.3",
                "preview/example/1.2.3",
                "preview/example/1.2.3.tar.gz",
                Some("https://packages.typst.org/preview/example-1.2.3.tar.gz"),
            ),
            (
                "@local/example:0.1.0",
                "local/example/0.1.0",
                "local/example/0.1.0.tar.gz",
                None,
            ),
            (
                "@preview/nested-name:10.0.1",
                "preview/nested-name/10.0.1",
                "preview/nested-name/10.0.1.tar.gz",
                Some("https://packages.typst.org/preview/nested-name-10.0.1.tar.gz"),
            ),
        ];

        for (text, tree, cache, registry) in layouts {
            let spec = spec(text);

            assert_eq!(package_tree_key(&spec), tree, "{text}");
            assert_eq!(package_archive_cache_key(&spec), cache, "{text}");
            assert_eq!(
                official_registry_archive_url(&spec).as_deref(),
                registry,
                "{text}"
            );
        }
    }

    #[test]
    fn only_the_served_namespace_has_an_official_registry_candidate() {
        for text in ["@local/example:1.0.0", "@custom/example:1.0.0"] {
            let unserved = spec(text);

            assert!(!official_registry_serves(&unserved), "{text}");
            assert_eq!(official_registry_archive_key(&unserved), None, "{text}");
            assert_eq!(official_registry_archive_url(&unserved), None, "{text}");
        }

        let served = spec("@preview/example:1.0.0");

        assert!(official_registry_serves(&served));
    }

    #[test]
    fn supported_font_container_extensions_are_matched_case_insensitively() {
        let extensions = [
            ("ttf", true),
            ("TTF", true),
            ("ttc", true),
            ("Ttc", true),
            ("otf", true),
            ("OTF", true),
            ("otc", true),
            ("oTc", true),
            ("woff", false),
            ("woff2", false),
            ("txt", false),
            ("", false),
            ("ttf ", false),
            (".ttf", false),
        ];

        for (extension, supported) in extensions {
            assert_eq!(
                is_font_container_extension(extension),
                supported,
                "{extension:?}"
            );
        }
    }

    /// A key-addressed source asks only whether a supported suffix terminates
    /// the key, so both rules select the same set apart from a key that is
    /// nothing but a suffix.
    #[test]
    fn a_key_names_a_font_container_when_a_supported_suffix_terminates_it() {
        let keys = [
            ("container.ttf", true),
            ("container.TTF", true),
            ("fonts/nested/container.otc", true),
            ("container.Ttc", true),
            ("container.oTf", true),
            (".ttf", true),
            ("container.woff", false),
            ("container.ttf.txt", false),
            ("container", false),
            ("ttf", false),
            ("fonts.ttf/container", false),
            ("", false),
        ];

        for (key, supported) in keys {
            assert_eq!(is_font_container_key(key), supported, "{key:?}");
        }
    }
}
