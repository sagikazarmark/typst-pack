use std::fmt;

/// Keeps a native source available without exposing it through an outer Debug.
#[derive(thiserror::Error)]
#[error("{0}")]
pub(crate) struct RedactedError<T: std::error::Error + 'static>(#[source] T);

impl<T: std::error::Error + 'static> RedactedError<T> {
    pub(crate) fn new(source: T) -> Self {
        Self(source)
    }

    pub(crate) const fn inner(&self) -> &T {
        &self.0
    }
}

impl<T: std::error::Error + 'static> fmt::Debug for RedactedError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}
