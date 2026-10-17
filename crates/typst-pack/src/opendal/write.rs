//! Exact-key writes of Pack Archives, package cache archives, extraction
//! plans, and compilation artifacts.

use std::collections::HashSet;

use ::opendal::{ErrorKind, Operator};
use futures_util::TryStreamExt;
use typst::syntax::package::PackageSpec;

use super::{Error, check_key, check_prefix, storage};
use crate::{
    CompilationResult, CompilationStatus, PackArchiveBytes, PackExtractionPlan, read_layout,
};

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
/// Writes are sequential and not atomic across keys. `completed` lists the
/// full paths (in input order) whose outcome was established before the
/// failure. `indeterminate` is `true` when the failing key's write was issued
/// and may or may not have taken effect. Replaying the whole write is the
/// recovery path; with [`WritePolicy::CreateOrVerify`] already written keys
/// complete as [`WriteOutcome::AlreadyMatching`].
#[derive(Debug, thiserror::Error)]
#[error("write failed after {} completed key(s)", .completed.len())]
pub struct WriteError {
    pub completed: Vec<(String, WriteOutcome)>,
    pub indeterminate: bool,
    #[source]
    pub error: Error,
}

impl From<Error> for WriteError {
    fn from(error: Error) -> Self {
        Self {
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
/// conflict without downloading; otherwise the object is streamed and
/// compared chunk by chunk. `Ok(None)` means the object is absent.
async fn matches(op: &Operator, path: &str, expected: &[u8]) -> Result<Option<bool>, Error> {
    let length = match op.stat(path).await {
        Ok(metadata) => metadata.content_length(),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(storage(path, "stat")(error)),
    };
    if length != expected.len() as u64 {
        return Ok(Some(false));
    }
    if length == 0 {
        return Ok(Some(true));
    }
    let mut stream = op
        .reader(path)
        .await
        .map_err(storage(path, "read"))?
        .into_stream(0..length)
        .await
        .map_err(storage(path, "read"))?;
    let mut offset = 0;
    while let Some(buffer) = stream.try_next().await.map_err(storage(path, "read"))? {
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

/// Writes `entries` (full paths and bytes) in order under `policy`.
async fn write_keys(
    op: &Operator,
    policy: WritePolicy,
    entries: &[(String, &[u8])],
) -> Result<Vec<(String, WriteOutcome)>, WriteError> {
    require_write(op, policy, entries)?;
    let mut completed = Vec::with_capacity(entries.len());
    let fail = |completed: Vec<_>, indeterminate, error| WriteError {
        completed,
        indeterminate,
        error,
    };

    if policy == WritePolicy::Overwrite {
        for (path, bytes) in entries {
            if let Err(error) = op.write(path, bytes.to_vec()).await {
                return Err(fail(completed, true, storage(path, "write")(error)));
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
                    Vec::new(),
                    false,
                    Error::Conflict { path: path.clone() },
                ));
            }
            Err(error) => return Err(fail(Vec::new(), false, error)),
        }
    }

    for ((path, bytes), absent) in entries.iter().zip(absent) {
        let outcome = if !absent {
            WriteOutcome::AlreadyMatching
        } else {
            match op
                .write_with(path, bytes.to_vec())
                .if_not_exists(true)
                .await
            {
                Ok(_) => WriteOutcome::Created,
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::AlreadyExists | ErrorKind::ConditionNotMatch
                    ) =>
                {
                    // Another writer created the object first; accept it only
                    // when its bytes match.
                    match matches(op, path, bytes).await {
                        Ok(Some(true)) => WriteOutcome::AlreadyMatching,
                        Ok(_) => {
                            let error = Error::Conflict { path: path.clone() };
                            return Err(fail(completed, false, error));
                        }
                        Err(error) => return Err(fail(completed, false, error)),
                    }
                }
                Err(error) => {
                    let error = storage(path, "conditional create")(error);
                    return Err(fail(completed, true, error));
                }
            }
        };
        completed.push((path.clone(), outcome));
    }
    Ok(completed)
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

/// Writes one Pack Archive object at `path`.
pub async fn write_pack_archive(
    op: &Operator,
    path: &str,
    archive: &PackArchiveBytes,
    policy: WritePolicy,
) -> Result<WriteOutcome, WriteError> {
    write_one(op, policy, path.to_owned(), archive.as_slice()).await
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
    let path = format!("{prefix}{}", read_layout::package_archive_cache_key(spec));
    write_one(op, WritePolicy::CreateOrVerify, path, bytes).await
}

/// Writes every entry of a Pack Extraction Plan below `prefix`.
pub async fn write_pack_extraction_plan(
    op: &Operator,
    prefix: &str,
    plan: &PackExtractionPlan,
    policy: WritePolicy,
) -> Result<Vec<(String, WriteOutcome)>, WriteError> {
    check_prefix(prefix)?;
    let entries = plan
        .entries()
        .iter()
        .map(|entry| (format!("{prefix}{}", entry.relative_path()), entry.bytes()))
        .collect::<Vec<_>>();
    write_keys(op, policy, &entries).await
}

/// Writes each artifact of a succeeded compilation result at
/// `{prefix}{key}`, pairing `keys` with the artifacts in order.
///
/// Keys must be relative, normalized, and distinct.
pub async fn write_compilation_artifacts(
    op: &Operator,
    prefix: &str,
    result: &CompilationResult,
    keys: &[&str],
    policy: WritePolicy,
) -> Result<Vec<(String, WriteOutcome)>, WriteError> {
    check_prefix(prefix)?;
    if result.status() != CompilationStatus::Succeeded {
        return Err(Error::ResultNotSucceeded.into());
    }
    if keys.len() != result.artifacts().len() {
        return Err(Error::ArtifactKeyCount {
            expected: result.artifacts().len(),
            actual: keys.len(),
        }
        .into());
    }
    let mut seen = HashSet::new();
    for key in keys {
        check_key(key)?;
        if !seen.insert(*key) {
            return Err(Error::InvalidPath {
                path: (*key).to_owned(),
                reason: "an artifact key is used more than once",
            }
            .into());
        }
    }
    let entries = keys
        .iter()
        .zip(result.artifacts())
        .map(|(key, artifact)| (format!("{prefix}{key}"), artifact.bytes()))
        .collect::<Vec<_>>();
    write_keys(op, policy, &entries).await
}
