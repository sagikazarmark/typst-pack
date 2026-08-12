#![no_main]

use libfuzzer_sys::fuzz_target;
use typst_pack::opendal::{Location, LocationError, LocationRoleError, OperatorBinding};

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let binding = OperatorBinding::new("fuzz").unwrap();
    let mut stem = data
        .iter()
        .take(64)
        .map(|byte| char::from(b'a' + byte % 26))
        .collect::<String>();
    if stem.is_empty() {
        stem.push('a');
    }

    for alias in [format!(" {stem}"), format!("{stem} ")] {
        assert!(matches!(
            Location::from_operation_path(binding.clone(), alias),
            Err(LocationError::NormalizationAlias { .. })
        ));
    }
    assert!(matches!(
        Location::parse(format!("fuzz:/{stem}%20")),
        Err(LocationError::NormalizationAlias { .. })
    ));

    if let Ok(location) = Location::parse(input) {
        let canonical = location.to_string();
        assert_eq!(Location::parse(&canonical).unwrap(), location);
        assert_eq!(format!("{location:?}"), canonical);
        check_roles(&location);
    }

    if let Ok(location) = Location::from_operation_path(binding, input) {
        assert_eq!(Location::parse(location.to_string()).unwrap(), location);
        check_roles(&location);

        if let Ok(child) = location.fuzz_compose("child") {
            assert!(!child.operation_path().starts_with('/'));
            if location.is_root() {
                assert_eq!(child.operation_path(), "child");
            }
        }
    }
});

fn check_roles(location: &Location) {
    let (object, prefix) = location.fuzz_role_checks();
    if location.is_root() {
        assert_eq!(object, Err(LocationRoleError::ObjectAtRoot));
        assert_eq!(prefix, Ok(()));
    } else if location.has_trailing_slash() {
        assert_eq!(object, Err(LocationRoleError::ObjectHasTrailingSlash));
        assert_eq!(prefix, Ok(()));
    } else {
        assert_eq!(object, Ok(()));
        assert_eq!(prefix, Err(LocationRoleError::PrefixMissingTrailingSlash));
    }
}
