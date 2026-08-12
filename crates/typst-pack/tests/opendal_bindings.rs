#![cfg(feature = "opendal")]

use std::{cell::Cell, error::Error, fmt, sync::Arc};

use typst_pack::opendal::{
    Operator, OperatorBinding, OperatorBindings, OperatorBindingsError,
    OperatorBindingsResolveError, OperatorResolver,
};

#[test]
fn bindings_iterate_lexically_and_report_the_exact_size() {
    let operator = memory_operator();
    let bindings = OperatorBindings::new([
        (binding("project"), operator.clone()),
        (binding("archive"), operator.clone()),
        (binding("cache"), operator),
    ])
    .unwrap();

    let names = bindings.bindings();
    assert_eq!(names.len(), 3);
    assert_eq!(
        names.map(OperatorBinding::as_str).collect::<Vec<_>>(),
        ["archive", "cache", "project"]
    );
}

#[test]
fn duplicate_bindings_are_rejected_with_safe_binding_identity() {
    let operator = memory_operator();
    let archive = binding("archive");

    let error = OperatorBindings::new([
        (archive.clone(), operator.clone()),
        (archive.clone(), operator),
    ])
    .unwrap_err();

    assert_eq!(
        error,
        OperatorBindingsError::DuplicateBinding {
            binding: archive.clone(),
        }
    );
    assert_eq!(error.to_string(), "duplicate Operator binding archive");
    assert_eq!(
        format!("{error:?}"),
        "DuplicateBinding { binding: archive }"
    );
}

#[test]
fn unknown_bindings_are_typed_and_retain_binding_identity() {
    let bindings = OperatorBindings::new([]).unwrap();
    let missing = binding("missing");

    assert!(bindings.operator(&missing).is_none());
    let error = bindings.resolve(&missing).unwrap_err();

    assert_eq!(
        error,
        OperatorBindingsResolveError::UnknownBinding {
            binding: missing.clone(),
        }
    );
    assert_eq!(error.to_string(), "unknown Operator binding missing");
    assert_eq!(format!("{error:?}"), "UnknownBinding { binding: missing }");
}

#[test]
fn aliases_and_binding_map_clones_reuse_caller_owned_operators() {
    let operator = memory_operator();
    let archive = binding("archive");
    let mirror = binding("mirror");
    let bindings = OperatorBindings::new([
        (archive.clone(), operator.clone()),
        (mirror.clone(), operator.clone()),
    ])
    .unwrap();
    let cloned_bindings = bindings.clone();

    let archive_operator = bindings.operator(&archive).unwrap();
    let mirror_operator = bindings.resolve(&mirror).unwrap();
    let cloned_operator = cloned_bindings.resolve(&archive).unwrap();

    assert!(Arc::ptr_eq(operator.service(), archive_operator.service()));
    assert!(Arc::ptr_eq(operator.service(), mirror_operator.service()));
    assert!(Arc::ptr_eq(operator.service(), cloned_operator.service()));
}

#[test]
fn bindings_debug_lists_only_binding_identity() {
    let bindings = OperatorBindings::new([
        (binding("project"), memory_operator()),
        (binding("archive"), memory_operator()),
    ])
    .unwrap();

    let debug = format!("{bindings:?}");
    assert_eq!(debug, "OperatorBindings { bindings: [archive, project] }");
    assert!(!debug.contains("Memory"));
    assert!(!debug.contains("Operator {"));
}

#[test]
fn custom_resolvers_keep_their_typed_error_and_receive_only_the_binding() {
    let resolver = RejectingResolver {
        calls: Cell::new(0),
    };
    let archive = binding("archive");

    let error = resolve_for_consumer(&resolver, &archive).unwrap_err();

    assert_eq!(resolver.calls.get(), 1);
    assert_eq!(error, CustomResolveError { binding: archive });
}

fn resolve_for_consumer<R: OperatorResolver>(
    resolver: &R,
    binding: &OperatorBinding,
) -> Result<Operator, R::Error> {
    resolver.resolve(binding)
}

struct RejectingResolver {
    calls: Cell<usize>,
}

impl OperatorResolver for RejectingResolver {
    type Error = CustomResolveError;

    fn resolve(&self, binding: &OperatorBinding) -> Result<Operator, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        Err(CustomResolveError {
            binding: binding.clone(),
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CustomResolveError {
    binding: OperatorBinding,
}

impl fmt::Display for CustomResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "custom resolver rejected {}", self.binding)
    }
}

impl Error for CustomResolveError {}

fn binding(value: &str) -> OperatorBinding {
    OperatorBinding::new(value).unwrap()
}

fn memory_operator() -> Operator {
    Operator::new(opendal::services::Memory::default()).unwrap()
}
