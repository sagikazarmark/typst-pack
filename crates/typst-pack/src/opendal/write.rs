//! Exact-key writes of pack archives, package cache archives, extraction
//! plans, and compiled artifacts.

use std::collections::HashSet;

use ::opendal::{ErrorKind, Operator};
use futures_util::TryStreamExt;
use typst::syntax::package::PackageSpec;

use super::{
    Error, archive_cache_key, check_key, check_prefix, key_issue, read_failed, stat, storage,
    stream,
};
use crate::compile::Artifact;
use crate::extract::Plan;
use crate::{Pack, archive};

/// How a write treats objects that already exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WritePolicy {
    /// Create absent objects; accept existing objects only when their bytes
    /// match, and fail with [`Error::Conflict`] otherwise. Every key is
    /// checked before any is created, so a conflict found up front leaves
    /// storage untouched. Replaying a partially completed write is safe.
    CreateOrVerify,
    /// Write every key, replacing whatever is there.
    Overwrite,
}

/// What a write did at one key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WriteOutcome {
    /// `CreateOrVerify` created the absent object.
    Created,
    /// `CreateOrVerify` found the exact bytes already present.
    AlreadyMatching,
    /// `Overwrite` wrote the object.
    Written,
}

/// A write that failed, with the keys it completed before failing.
///
/// Writes are sequential and not atomic across keys. `path` names the key
/// that failed, or is `None` when the write was refused before any key was
/// tried. `completed` lists the full paths (in input order) whose outcome was
/// established before the failure. `indeterminate` is `true` when the failing
/// key's write was issued and may or may not have taken effect. Replaying the
/// whole write is the recovery path; with [`WritePolicy::CreateOrVerify`]
/// already written keys complete as [`WriteOutcome::AlreadyMatching`].
#[derive(Debug, thiserror::Error)]
#[error("{}", summary(.path.as_deref(), .completed.len()))]
#[non_exhaustive]
pub struct WriteError {
    /// The full path of the key whose write failed.
    pub path: Option<String>,
    /// The keys whose outcome was established before the failure, in order.
    pub completed: Vec<(String, WriteOutcome)>,
    /// Whether the failing key's write was issued and may have taken effect.
    pub indeterminate: bool,
    /// Why the write failed.
    #[source]
    pub error: Error,
}

fn summary(path: Option<&str>, completed: usize) -> String {
    match path {
        Some(path) => format!("writing {path:?} failed after {completed} completed key(s)"),
        None => "nothing was written".to_owned(),
    }
}

impl From<Error> for WriteError {
    fn from(error: Error) -> Self {
        Self {
            path: None,
            completed: Vec::new(),
            indeterminate: false,
            error,
        }
    }
}

fn require_write(
    op: &Operator,
    policy: WritePolicy,
    entries: &[(String, &[u8])],
) -> Result<(), Error> {
    let capability = op.info().capability();
    let unsupported = |capability| Err(Error::Unsupported { capability });
    if !capability.write {
        return unsupported("write");
    }
    if !capability.write_can_empty && entries.iter().any(|(_, bytes)| bytes.is_empty()) {
        return unsupported("writing empty objects");
    }
    if let Some(maximum) = capability.write_total_max_size
        && entries.iter().any(|(_, bytes)| bytes.len() > maximum)
    {
        return unsupported("writing objects of this size");
    }
    if policy == WritePolicy::CreateOrVerify
        && !(capability.stat && capability.read && capability.write_with_if_not_exists)
    {
        return unsupported("conditional create (stat, read, and write if-not-exists)");
    }
    Ok(())
}

/// Whether an existing object matches `expected`. A length mismatch is a
/// conflict without downloading; otherwise the whole object is streamed (in
/// one request, conditional on the stat's ETag where supported) and compared
/// chunk by chunk, so an object that grew after the stat never matches on
/// its prefix. `Ok(None)` means the object is absent.
async fn matches(op: &Operator, path: &str, expected: &[u8]) -> Result<Option<bool>, Error> {
    let Some(metadata) = stat(op, path).await? else {
        return Ok(None);
    };
    if metadata.content_length() != expected.len() as u64 {
        return Ok(Some(false));
    }
    let mut chunks = stream(op, path, &metadata).await?;
    let mut offset = 0;
    while let Some(buffer) = chunks.try_next().await.map_err(read_failed(path))? {
        for chunk in buffer {
            let end = offset + chunk.len();
            if expected.get(offset..end) != Some(&chunk[..]) {
                return Ok(Some(false));
            }
            offset = end;
        }
    }
    Ok(Some(offset == expected.len()))
}

/// The `CreateOrVerify` write of one absent key. The conditional create is
/// retried once when it is refused although the object turns out absent (for
/// example S3's `ConditionalRequestConflict` while another upload is in
/// flight); a second such refusal is a retryable storage error, not a
/// conflict. `Err` carries whether the write may have taken effect.
async fn create(op: &Operator, path: &str, bytes: &[u8]) -> Result<WriteOutcome, (bool, Error)> {
    for _ in 0..2 {
        match op
            .write_with(path, bytes.to_vec())
            .if_not_exists(true)
            .await
        {
            Ok(_) => return Ok(WriteOutcome::Created),
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::AlreadyExists | ErrorKind::ConditionNotMatch
                ) =>
            {
                // Another writer created the object first; accept it only
                // when its bytes match.
                match matches(op, path, bytes).await {
                    Ok(Some(true)) => return Ok(WriteOutcome::AlreadyMatching),
                    Ok(Some(false)) => {
                        return Err((
                            false,
                            Error::Conflict {
                                path: path.to_owned(),
                            },
                        ));
                    }
                    // Absent after all: try the conditional create again.
                    Ok(None) => {}
                    Err(error) => return Err((false, error)),
                }
            }
            Err(error) => return Err((true, storage(path, "conditional create")(error))),
        }
    }
    let refused = ::opendal::Error::new(
        ErrorKind::ConditionNotMatch,
        "the conditional create was refused twice but the object is absent",
    )
    .set_temporary();
    Err((false, storage(path, "conditional create")(refused)))
}

/// Writes `entries` (full paths and bytes) in order under `policy`.
async fn write_keys(
    op: &Operator,
    policy: WritePolicy,
    entries: &[(String, &[u8])],
) -> Result<Vec<(String, WriteOutcome)>, WriteError> {
    require_write(op, policy, entries)?;
    let mut completed = Vec::with_capacity(entries.len());
    let fail = |path: &String, completed: Vec<_>, indeterminate, error| WriteError {
        path: Some(path.clone()),
        completed,
        indeterminate,
        error,
    };

    if policy == WritePolicy::Overwrite {
        for (path, bytes) in entries {
            if let Err(error) = op.write(path, bytes.to_vec()).await {
                return Err(fail(path, completed, true, storage(path, "write")(error)));
            }
            completed.push((path.clone(), WriteOutcome::Written));
        }
        return Ok(completed);
    }

    // Check every key before creating any, so a conflict anywhere is
    // reported before storage changes.
    let mut absent = Vec::with_capacity(entries.len());
    for (path, bytes) in entries {
        match matches(op, path, bytes).await {
            Ok(None) => absent.push(true),
            Ok(Some(true)) => absent.push(false),
            Ok(Some(false)) => {
                return Err(fail(
                    path,
                    Vec::new(),
                    false,
                    Error::Conflict { path: path.clone() },
                ));
            }
            Err(error) => return Err(fail(path, Vec::new(), false, error)),
        }
    }

    for ((path, bytes), absent) in entries.iter().zip(absent) {
        let outcome = if absent {
            match create(op, path, bytes).await {
                Ok(outcome) => outcome,
                Err((indeterminate, error)) => {
                    return Err(fail(path, completed, indeterminate, error));
                }
            }
        } else {
            WriteOutcome::AlreadyMatching
        };
        completed.push((path.clone(), outcome));
    }
    Ok(completed)
}

/// Joins `prefix` to every key after checking all of them, so a key that
/// OpenDAL would normalize into another key (or out of the prefix) or a
/// repeated key is refused, with every offending key listed, before anything
/// is written.
fn join_keys<'a>(
    prefix: &str,
    entries: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<Vec<(String, &'a [u8])>, Error> {
    let mut seen = HashSet::new();
    let mut invalid = Vec::new();
    let mut joined = Vec::new();
    for (key, bytes) in entries {
        if let Some(reason) = key_issue(key) {
            invalid.push((key.to_owned(), reason));
        } else if !seen.insert(key) {
            invalid.push((key.to_owned(), "an object key is used more than once"));
        }
        joined.push((format!("{prefix}{key}"), bytes));
    }
    if invalid.is_empty() {
        Ok(joined)
    } else {
        Err(Error::InvalidKeys { keys: invalid })
    }
}

async fn write_one(
    op: &Operator,
    policy: WritePolicy,
    path: String,
    bytes: &[u8],
) -> Result<WriteOutcome, WriteError> {
    check_key(&path)?;
    let mut completed = write_keys(op, policy, &[(path, bytes)]).await?;
    Ok(completed.pop().expect("one key was written").1)
}

/// Encodes `pack` and writes it as one pack archive object at `path`.
pub async fn write_archive(
    op: &Operator,
    path: &str,
    pack: &Pack,
    policy: WritePolicy,
) -> Result<WriteOutcome, WriteError> {
    write_one(op, policy, path.to_owned(), &archive::encode(pack)).await
}

/// Writes raw package archive bytes into a package archive cache at
/// `{prefix}{namespace}/{name}/{version}.tar.gz`, the layout
/// [`super::PackageSource::ArchiveCache`] reads.
///
/// The write always uses [`WritePolicy::CreateOrVerify`]. Write only bytes
/// that already expanded and validated (for example after
/// [`super::insert_package`] succeeded); caching malformed bytes would turn a
/// transient failure into a permanent one.
pub async fn write_package_archive(
    op: &Operator,
    prefix: &str,
    spec: &PackageSpec,
    bytes: &[u8],
) -> Result<WriteOutcome, WriteError> {
    check_prefix(prefix)?;
    let path = format!("{prefix}{}", archive_cache_key(spec));
    write_one(op, WritePolicy::CreateOrVerify, path, bytes).await
}

/// Writes every entry of an extraction plan below `prefix`.
///
/// Every entry path must be a normalized object key (pack paths may end in
/// whitespace, which OpenDAL would trim into another key); otherwise the write
/// fails with [`Error::InvalidKeys`] listing them before anything is written.
pub async fn write_extraction(
    op: &Operator,
    prefix: &str,
    plan: &Plan,
    policy: WritePolicy,
) -> Result<Vec<(String, WriteOutcome)>, WriteError> {
    check_prefix(prefix)?;
    let entries = join_keys(
        prefix,
        plan.entries()
            .iter()
            .map(|entry| (entry.path(), entry.bytes())),
    )?;
    write_keys(op, policy, &entries).await
}

/// Writes each compiled artifact at `{prefix}{key}`, pairing `keys` with the
/// artifacts in order.
///
/// Keys must be relative, normalized, and distinct; otherwise the write fails
/// with [`Error::InvalidKeys`] listing them before anything is written.
pub async fn write_artifacts(
    op: &Operator,
    prefix: &str,
    keys: &[impl AsRef<str>],
    artifacts: &[Artifact],
    policy: WritePolicy,
) -> Result<Vec<(String, WriteOutcome)>, WriteError> {
    check_prefix(prefix)?;
    if keys.len() != artifacts.len() {
        return Err(Error::ArtifactKeyCount {
            expected: artifacts.len(),
            actual: keys.len(),
        }
        .into());
    }
    let entries = join_keys(
        prefix,
        keys.iter()
            .zip(artifacts)
            .map(|(key, artifact)| (key.as_ref(), artifact.bytes.as_slice())),
    )?;
    write_keys(op, policy, &entries).await
}
