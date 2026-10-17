/// Knowledge about whether one attempted destination effect completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitCertainty {
    NotCommitted,
    Committed,
    Indeterminate,
}
