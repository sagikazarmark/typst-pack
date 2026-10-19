//! A small fault-injecting wrapper around a real OpenDAL service.
//!
//! [`Faults::wrap`] returns an operator that forwards every call to the given
//! operator's service, except for the faults armed on the shared [`Faults`].
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use opendal::raw::{
    OpCompose, OpCopy, OpCreateDir, OpList, OpPresign, OpRead, OpRename, OpRestore, OpStat,
    OpWrite, RpCreateDir, RpPresign, RpRename, RpRestore, RpStat, Service, ServiceInfo, Servicer,
    oio,
};
use opendal::{Buffer, Capability, Error, ErrorKind, Metadata, OperationContext, Operator, Result};

/// Faults armed on a wrapped operator, keyed by normalized object path.
pub struct Faults {
    /// The wrapped operator, used to change objects behind the reader's back.
    target: Operator,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// `stat` and `read` report these objects absent, though listings still
    /// yield them.
    vanished: HashSet<String>,
    /// The next `stat` of each path reports it absent.
    stale_stat_once: HashSet<String>,
    /// `read` fails with a temporary error.
    failing_reads: HashSet<String>,
    /// `write` fails with a temporary error before any effect.
    failing_writes: HashSet<String>,
    /// Capabilities removed from the advertised set.
    without_if_not_exists: bool,
    /// The next `stat` of each path returns the current metadata and then
    /// replaces the object with these bytes.
    replace_after_stat: HashMap<String, Vec<u8>>,
    /// Whether `stat` reports an ETag (the path's replacement count) and
    /// `read` honors `if_match`, as S3 does.
    etags: bool,
    /// Replacements made per path, the emulated ETag.
    generations: HashMap<String, u32>,
    /// Conditional creates refused with `ConditionNotMatch` and no effect,
    /// with the number of refusals left.
    refused_creates: HashMap<String, u32>,
    /// The next write of each path takes effect, then reports a failure.
    lost_acks: HashSet<String>,
}

fn injected(kind: ErrorKind, message: &'static str) -> Error {
    Error::new(kind, message)
}

impl Faults {
    /// Wraps `operator`, returning the faulty operator and its fault controls.
    pub fn wrap(operator: &Operator) -> (Operator, Arc<Self>) {
        let faults = Arc::new(Self {
            target: operator.clone(),
            state: Mutex::default(),
        });
        let (context, inner) = operator.clone().into_parts();
        let service = FaultService {
            inner,
            faults: faults.clone(),
        };
        (Operator::from_parts(context, Arc::new(service)), faults)
    }

    pub fn vanish(&self, path: &str) {
        self.state.lock().unwrap().vanished.insert(path.to_owned());
    }

    pub fn stale_stat_once(&self, path: &str) {
        self.state
            .lock()
            .unwrap()
            .stale_stat_once
            .insert(path.to_owned());
    }

    pub fn fail_read(&self, path: &str) {
        self.state
            .lock()
            .unwrap()
            .failing_reads
            .insert(path.to_owned());
    }

    pub fn fail_write(&self, path: &str) {
        self.state
            .lock()
            .unwrap()
            .failing_writes
            .insert(path.to_owned());
    }

    pub fn hide_conditional_create(&self) {
        self.state.lock().unwrap().without_if_not_exists = true;
    }

    pub fn replace_after_stat(&self, path: &str, bytes: impl Into<Vec<u8>>) {
        self.state
            .lock()
            .unwrap()
            .replace_after_stat
            .insert(path.to_owned(), bytes.into());
    }

    pub fn emulate_etags(&self) {
        self.state.lock().unwrap().etags = true;
    }

    pub fn refuse_creates(&self, path: &str, times: u32) {
        self.state
            .lock()
            .unwrap()
            .refused_creates
            .insert(path.to_owned(), times);
    }

    pub fn lose_ack(&self, path: &str) {
        self.state.lock().unwrap().lost_acks.insert(path.to_owned());
    }
}

struct FaultService {
    inner: Servicer,
    faults: Arc<Faults>,
}

impl fmt::Debug for FaultService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FaultService")
            .finish_non_exhaustive()
    }
}

impl FaultService {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.faults.state.lock().unwrap()
    }
}

impl Service for FaultService {
    type Reader = oio::Reader;
    type Writer = oio::Writer;
    type Lister = oio::Lister;
    type Deleter = oio::Deleter;
    type Copier = oio::Copier;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.inner.info_dyn()
    }

    fn capability(&self) -> Capability {
        let mut capability = self.inner.capability_dyn();
        let state = self.state();
        if state.without_if_not_exists {
            capability.write_with_if_not_exists = false;
        }
        if state.etags {
            capability.read_with_if_match = true;
        }
        capability
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.inner.create_dir_dyn(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let (etag, replacement) = {
            let mut state = self.state();
            if state.vanished.contains(path) || state.stale_stat_once.remove(path) {
                return Err(injected(ErrorKind::NotFound, "injected: object absent"));
            }
            let etag = state
                .etags
                .then(|| format!("\"{}\"", state.generations.get(path).unwrap_or(&0)));
            (etag, state.replace_after_stat.remove(path))
        };
        let mut metadata: Metadata = self.inner.stat_dyn(ctx, path, args).await?.into_metadata();
        if let Some(etag) = etag {
            let mut builder = metadata.into_builder();
            builder.etag(etag);
            metadata = builder.build();
        }
        if let Some(bytes) = replacement {
            self.faults.target.write(path, bytes).await?;
            *self.state().generations.entry(path.to_owned()).or_default() += 1;
        }
        Ok(RpStat::new(metadata))
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<oio::Reader> {
        let state = self.state();
        if state.vanished.contains(path) {
            return Err(injected(ErrorKind::NotFound, "injected: object absent"));
        }
        if state.failing_reads.contains(path) {
            return Err(injected(ErrorKind::Unexpected, "injected: read failed").set_temporary());
        }
        if let Some(etag) = args.if_match() {
            let current = format!("\"{}\"", state.generations.get(path).unwrap_or(&0));
            if etag != current {
                return Err(injected(
                    ErrorKind::ConditionNotMatch,
                    "injected: etag does not match",
                ));
            }
            // The wrapped service does not support conditional reads.
            return self.inner.read_dyn(ctx, path, OpRead::new());
        }
        drop(state);
        self.inner.read_dyn(ctx, path, args)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<oio::Writer> {
        let mut state = self.state();
        if state.failing_writes.contains(path) {
            return Err(injected(ErrorKind::Unexpected, "injected: write failed").set_temporary());
        }
        if args.if_not_exists()
            && let Some(left) = state.refused_creates.get_mut(path)
            && *left > 0
        {
            *left -= 1;
            return Err(injected(
                ErrorKind::ConditionNotMatch,
                "injected: conditional create refused",
            ));
        }
        let lost_ack = state.lost_acks.remove(path);
        drop(state);
        let writer = self.inner.write_dyn(ctx, path, args)?;
        Ok(if lost_ack {
            Box::new(LostAck(writer))
        } else {
            writer
        })
    }

    fn delete(&self, ctx: &OperationContext) -> Result<oio::Deleter> {
        self.inner.delete_dyn(ctx)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<oio::Lister> {
        self.inner.list_dyn(ctx, path, args)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<oio::Copier> {
        self.inner.copy_dyn(ctx, from, to, args)
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<oio::Composer> {
        self.inner.compose_dyn(ctx, to, args)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.inner.rename_dyn(ctx, from, to, args).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.inner.restore_dyn(ctx, path, args).await
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.inner.presign_dyn(ctx, path, args).await
    }
}

/// A writer whose writes take effect but whose close reports a failure, as
/// when a response is lost after the service committed the object.
struct LostAck(oio::Writer);

impl oio::Write for LostAck {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        oio::Write::write(&mut self.0, bs).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        oio::Write::close(&mut self.0).await?;
        Err(injected(ErrorKind::Unexpected, "injected: response lost").set_temporary())
    }

    async fn abort(&mut self) -> Result<()> {
        oio::Write::abort(&mut self.0).await
    }
}

/// Polls a future to completion on the current thread. OpenDAL's Memory
/// service never parks, so a no-op waker suffices.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}
