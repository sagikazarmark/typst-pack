use futures_util::StreamExt;
use opendal::ErrorKind;

use super::location::{OperatorBinding, OperatorResolver};

pub(crate) mod recursive;

#[derive(Clone)]
pub(crate) struct ResolvedOperator {
    pub(crate) operator: opendal::Operator,
    pub(crate) list: bool,
    pub(crate) list_with_recursive: bool,
    pub(crate) read: bool,
}

/// Resolves and appraises each reached binding once for one composite operation.
pub(crate) struct ResolvedOperators<'a, R: OperatorResolver + ?Sized> {
    resolver: &'a R,
    entries: Vec<(OperatorBinding, ResolvedOperator)>,
}

impl<'a, R: OperatorResolver + ?Sized> ResolvedOperators<'a, R> {
    pub(crate) fn new(resolver: &'a R) -> Self {
        Self {
            resolver,
            entries: Vec::new(),
        }
    }

    pub(crate) fn resolve(
        &mut self,
        binding: &OperatorBinding,
    ) -> Result<ResolvedOperator, R::Error> {
        if let Some((_, resolved)) = self
            .entries
            .iter()
            .find(|(candidate, _)| candidate == binding)
        {
            return Ok(resolved.clone());
        }

        let operator = self.resolver.resolve(binding)?;
        let capabilities = operator.info().capability();
        let resolved = ResolvedOperator {
            operator,
            list: capabilities.list,
            list_with_recursive: capabilities.list_with_recursive,
            read: capabilities.read,
        };
        self.entries.push((binding.clone(), resolved.clone()));
        Ok(resolved)
    }
}

pub(crate) trait ExactPathReadOperation {
    type Error;

    fn read(&self, source: opendal::Error) -> Self::Error;
    fn limit_exceeded(&self, ceiling: u64, observed_at_least: u64) -> Self::Error;
    fn accounting_overflow(&self) -> Self::Error;
}

pub(crate) async fn read_exact_path<O: ExactPathReadOperation>(
    operator: &opendal::Operator,
    operation_path: &str,
    retention_ceiling: u64,
    observation_ceiling: u64,
    operation: &O,
) -> Result<Option<Vec<u8>>, O::Error> {
    debug_assert!(retention_ceiling <= observation_ceiling);
    let probe_end = retention_ceiling
        .checked_add(1)
        .ok_or_else(|| operation.accounting_overflow())?;
    let observation_end = observation_ceiling
        .checked_add(1)
        .ok_or_else(|| operation.accounting_overflow())?;
    let reader = match operator.reader(operation_path).await {
        Ok(reader) => reader,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(operation.read(source)),
    };
    let mut stream = match reader.into_stream(..).await {
        Ok(stream) => stream,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(operation.read(source)),
    };
    let mut bytes = Vec::new();
    let mut observed = 0u64;
    let mut yielded_buffer = false;

    while let Some(buffer) = stream.next().await {
        let buffer = match buffer {
            Ok(buffer) => buffer,
            Err(source) if !yielded_buffer && source.kind() == ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(source) => return Err(operation.read(source)),
        };
        yielded_buffer = true;
        let buffer_len =
            u64::try_from(buffer.len()).map_err(|_| operation.accounting_overflow())?;
        let retained_so_far =
            u64::try_from(bytes.len()).map_err(|_| operation.accounting_overflow())?;
        let retained = probe_end
            .checked_sub(retained_so_far)
            .ok_or_else(|| operation.accounting_overflow())?
            .min(buffer_len);
        let retained = usize::try_from(retained).map_err(|_| operation.accounting_overflow())?;

        let mut remaining = retained;
        for chunk in buffer {
            let take = remaining.min(chunk.len());
            bytes.extend_from_slice(&chunk[..take]);
            remaining -= take;
            if remaining == 0 {
                break;
            }
        }
        let observed_here = observation_end
            .checked_sub(observed)
            .ok_or_else(|| operation.accounting_overflow())?
            .min(buffer_len);
        observed = observed
            .checked_add(observed_here)
            .ok_or_else(|| operation.accounting_overflow())?;
        if observed > observation_ceiling {
            return Err(operation.limit_exceeded(retention_ceiling, observation_end));
        }
    }

    if observed > retention_ceiling {
        Err(operation.limit_exceeded(retention_ceiling, observed))
    } else {
        Ok(Some(bytes))
    }
}

pub(crate) fn exact_path_absent_error() -> opendal::Error {
    opendal::Error::new(ErrorKind::NotFound, "the exact object is absent")
}
