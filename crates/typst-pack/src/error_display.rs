pub(crate) fn format_error_list(issues: &[impl std::fmt::Display]) -> String {
    use std::fmt::Write as _;

    let mut rendered = String::new();
    for issue in issues {
        write!(rendered, ": {issue}").expect("writing to a String cannot fail");
    }
    rendered
}
