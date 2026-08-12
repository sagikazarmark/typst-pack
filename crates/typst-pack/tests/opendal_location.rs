#![cfg(feature = "opendal")]

use proptest::prelude::*;
use typst_pack::opendal::{
    Location, LocationError, OperatorBinding, OperatorBindingError, OperatorBindings,
    OperatorBindingsError, OperatorBindingsResolveError, OperatorResolver,
};

#[test]
fn operator_bindings_are_canonical_lowercase_scheme_names() {
    let mut bindings =
        ["z-store", "archive.v1", "a+cache"].map(|value| OperatorBinding::new(value).unwrap());
    bindings.sort();

    assert_eq!(
        bindings.each_ref().map(ToString::to_string),
        ["a+cache", "archive.v1", "z-store"]
    );
    assert_eq!(format!("{:?}", bindings[0]), "a+cache");

    let cases = [
        ("", OperatorBindingError::Empty),
        (
            "Store",
            OperatorBindingError::NonLowercaseCharacter {
                index: 0,
                character: 'S',
            },
        ),
        (
            "sTore",
            OperatorBindingError::NonLowercaseCharacter {
                index: 1,
                character: 'T',
            },
        ),
        (
            "1store",
            OperatorBindingError::InvalidInitialCharacter {
                index: 0,
                character: '1',
            },
        ),
        (
            "store_1",
            OperatorBindingError::InvalidCharacter {
                index: 5,
                character: '_',
            },
        ),
    ];

    for (input, expected) in cases {
        assert_eq!(OperatorBinding::new(input), Err(expected), "{input:?}");
    }
}

#[test]
fn locations_round_trip_canonical_uri_spelling_and_decoded_paths() {
    let cases = [
        ("store:/", "", true, true),
        (
            "store:/objects/file.typk",
            "objects/file.typk",
            false,
            false,
        ),
        ("store:/objects/", "objects/", false, true),
        (
            "store:/caf%C3%A9/space%20and%25",
            "café/space and%",
            false,
            false,
        ),
        (
            "store:/literal:@!$&'()*+,;=-._~",
            "literal:@!$&'()*+,;=-._~",
            false,
            false,
        ),
        (
            "store:/%EF%BB%BF%E2%80%8B",
            "\u{feff}\u{200b}",
            false,
            false,
        ),
    ];

    for (spelling, operation_path, is_root, trailing_slash) in cases {
        let location = Location::parse(spelling).unwrap();
        assert_eq!(location.binding().as_str(), "store");
        assert_eq!(location.operation_path(), operation_path);
        assert_eq!(location.is_root(), is_root);
        assert_eq!(location.has_trailing_slash(), trailing_slash);
        assert_eq!(location.to_string(), spelling);
        assert_eq!(format!("{location:?}"), spelling);
        assert_eq!(spelling.parse::<Location>().unwrap(), location);
    }

    let binding = OperatorBinding::new("store").unwrap();
    for (path, spelling) in [
        ("", "store:/"),
        ("/", "store:/"),
        ("café/space and%", "store:/caf%C3%A9/space%20and%25"),
        ("question?#", "store:/question%3F%23"),
    ] {
        let location = Location::from_operation_path(binding.clone(), path).unwrap();
        assert_eq!(location.to_string(), spelling);
    }
}

#[test]
fn location_parsing_rejects_noncanonical_and_unsafe_spellings_with_byte_offsets() {
    let cases = [
        ("store", LocationError::MissingBindingSeparator),
        (
            "Store:/x",
            LocationError::InvalidBinding {
                source: OperatorBindingError::NonLowercaseCharacter {
                    index: 0,
                    character: 'S',
                },
            },
        ),
        ("store:x", LocationError::MissingAbsolutePath { index: 6 }),
        (
            "store://host/x",
            LocationError::AuthorityNotAllowed { index: 7 },
        ),
        (
            "store://user@host/x",
            LocationError::UserInfoNotAllowed { index: 12 },
        ),
        ("store:/é?x", LocationError::QueryNotAllowed { index: 9 }),
        ("store:/a#x", LocationError::FragmentNotAllowed { index: 8 }),
        ("store:/é", LocationError::RawNonAscii { index: 7 }),
        (
            "store:/a\u{7f}",
            LocationError::ControlCharacter { index: 8 },
        ),
        ("store:/a\\b", LocationError::Backslash { index: 8 }),
        (
            "store:/a%2g",
            LocationError::MalformedPercentEscape { index: 8 },
        ),
        (
            "store:/a%2f",
            LocationError::NoncanonicalPercentEscape { index: 8 },
        ),
        ("store:/a%2F", LocationError::EncodedSeparator { index: 8 }),
        ("store:/a%5C", LocationError::EncodedBackslash { index: 8 }),
        ("store:/a%41", LocationError::EncodedPchar { index: 8 }),
        ("store:/%80", LocationError::InvalidUtf8 { index: 7 }),
        ("store:/a//b", LocationError::RepeatedSeparator { index: 9 }),
        ("store:/a/../b", LocationError::DotSegment { index: 9 }),
        (
            "store:/%20a",
            LocationError::NormalizationAlias { index: 7 },
        ),
        (
            "store:/a b",
            LocationError::NoncanonicalPathCharacter {
                index: 8,
                character: ' ',
            },
        ),
    ];

    for (input, expected) in cases {
        assert_eq!(Location::parse(input), Err(expected), "{input:?}");
    }
}

#[test]
fn decoded_operation_paths_apply_the_same_safety_and_normalization_rules() {
    let binding = OperatorBinding::new("store").unwrap();
    let cases = [
        ("/child", LocationError::MissingAbsolutePath { index: 0 }),
        ("a//b", LocationError::RepeatedSeparator { index: 2 }),
        ("a/./b", LocationError::DotSegment { index: 2 }),
        ("a/../b", LocationError::DotSegment { index: 2 }),
        (" a", LocationError::NormalizationAlias { index: 0 }),
        ("a ", LocationError::NormalizationAlias { index: 1 }),
        ("a\\b", LocationError::Backslash { index: 1 }),
        ("a\0b", LocationError::ControlCharacter { index: 1 }),
    ];

    for (input, expected) in cases {
        assert_eq!(
            Location::from_operation_path(binding.clone(), input),
            Err(expected),
            "{input:?}"
        );
    }
}

#[test]
fn normalization_aliases_follow_char_is_whitespace_exactly() {
    let binding = OperatorBinding::new("store").unwrap();
    let whitespace = [
        '\u{0009}', '\u{000a}', '\u{000b}', '\u{000c}', '\u{000d}', '\u{0020}', '\u{0085}',
        '\u{00a0}', '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}',
        '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}',
        '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
    ];

    for character in whitespace {
        assert!(character.is_whitespace());
        let path = format!("a{character}");
        let expected = if character.is_control() {
            LocationError::ControlCharacter { index: 1 }
        } else {
            LocationError::NormalizationAlias { index: 1 }
        };
        assert_eq!(
            Location::from_operation_path(binding.clone(), &path),
            Err(expected),
            "U+{:04X}",
            u32::from(character)
        );
    }

    for character in ['\u{feff}', '\u{200b}'] {
        assert!(!character.is_whitespace());
        let path = format!("a{character}");
        let location = Location::from_operation_path(binding.clone(), &path).unwrap();
        assert_eq!(location.operation_path(), path);
    }
}

#[test]
fn location_error_categories_have_deterministic_precedence() {
    let cases = [
        ("store:/é?x", LocationError::QueryNotAllowed { index: 9 }),
        ("store:/é#x", LocationError::FragmentNotAllowed { index: 9 }),
        ("store:/é\\x", LocationError::RawNonAscii { index: 7 }),
        (
            "store:/\u{7f}\\x",
            LocationError::ControlCharacter { index: 7 },
        ),
        ("store:/\\%", LocationError::Backslash { index: 7 }),
        (
            "store:/%G0%2f",
            LocationError::MalformedPercentEscape { index: 7 },
        ),
        (
            "store:/%2f%2F",
            LocationError::NoncanonicalPercentEscape { index: 7 },
        ),
        (
            "store:/%2F%5C",
            LocationError::EncodedSeparator { index: 7 },
        ),
        (
            "store:/%5C%41",
            LocationError::EncodedBackslash { index: 7 },
        ),
        ("store:/%41//b", LocationError::EncodedPchar { index: 7 }),
        (
            "store:/a//%80",
            LocationError::RepeatedSeparator { index: 9 },
        ),
        ("store:/a/../%80", LocationError::InvalidUtf8 { index: 12 }),
    ];

    for (input, expected) in cases {
        assert_eq!(Location::parse(input), Err(expected), "{input:?}");
    }
}

#[test]
fn operator_bindings_are_immutable_lexical_maps_of_cheap_operator_clones() {
    let operator = opendal::Operator::new(opendal::services::Memory::default()).unwrap();
    let archive = OperatorBinding::new("archive").unwrap();
    let project = OperatorBinding::new("project").unwrap();
    let bindings = OperatorBindings::new([
        (project.clone(), operator.clone()),
        (archive.clone(), operator.clone()),
    ])
    .unwrap();

    assert_eq!(
        bindings
            .bindings()
            .map(OperatorBinding::as_str)
            .collect::<Vec<_>>(),
        ["archive", "project"]
    );
    assert!(bindings.operator(&archive).is_some());
    assert!(bindings.resolve(&project).is_ok());
    assert_eq!(
        format!("{bindings:?}"),
        "OperatorBindings { bindings: [archive, project] }"
    );

    let duplicate = OperatorBindings::new([
        (archive.clone(), operator.clone()),
        (archive.clone(), operator),
    ])
    .unwrap_err();
    assert_eq!(
        duplicate,
        OperatorBindingsError::DuplicateBinding {
            binding: archive.clone(),
        }
    );

    let missing = OperatorBinding::new("missing").unwrap();
    assert!(bindings.operator(&missing).is_none());
    assert!(matches!(
        bindings.resolve(&missing),
        Err(OperatorBindingsResolveError::UnknownBinding { binding }) if binding == missing
    ));
}

proptest! {
    #[test]
    fn decoded_safe_paths_round_trip_through_canonical_display(
        segments in prop::collection::vec("[a-zA-Z0-9_~!$&'()*+,;=:@-]{1,16}", 1..8),
        trailing_slash in any::<bool>(),
    ) {
        let mut operation_path = segments.join("/");
        if trailing_slash {
            operation_path.push('/');
        }
        let binding = OperatorBinding::new("store").unwrap();
        let location = Location::from_operation_path(binding, &operation_path).unwrap();
        let reparsed = Location::parse(location.to_string()).unwrap();

        prop_assert_eq!(&reparsed, &location);
        prop_assert_eq!(location.operation_path(), operation_path);
    }
}
