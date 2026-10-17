//! A small fault-injecting wrapper around a real OpenDAL service.
//!
//! [`Faults::wrap`] returns an operator that forwards every call to the given
//! operator's service, except for the faults armed on the shared [`Faults`].
#![allow(dead_code)]

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

use opendal::raw::{
    OpCopier, OpCopy, OpCreateDir, OpList, OpPresign, OpRead, OpRename, OpStat, OpWrite,
    RpCreateDir, RpPresign, RpRename, RpStat, Service, ServiceInfo, Servicer, oio,
};
use opendal::{Capability, Error, ErrorKind, OperationContext, Operator, Result};

/// Faults armed on a wrapped operator, keyed by normalized object path.
#[derive(Default)]
pub struct Faults {
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
}

fn injected(kind: ErrorKind, message: &'static str) -> Error {
    Error::new(kind, message)
}

impl Faults {
    /// Wraps `operator`, returning the faulty operator and its fault controls.
    pub fn wrap(operator: &Operator) -> (Operator, Arc<Self>) {
        let faults = Arc::new(Self::default());
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

    fn info(&self) -> ServiceInfo {
        self.inner.info_dyn()
    }

    fn capability(&self) -> Capability {
        let mut capability = self.inner.capability_dyn();
        if self.state().without_if_not_exists {
            capability.write_with_if_not_exists = false;
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
        {
            let mut state = self.state();
            if state.vanished.contains(path) || state.stale_stat_once.remove(path) {
                return Err(injected(ErrorKind::NotFound, "injected: object absent"));
            }
        }
        self.inner.stat_dyn(ctx, path, args).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<oio::Reader> {
        {
            let state = self.state();
            if state.vanished.contains(path) {
                return Err(injected(ErrorKind::NotFound, "injected: object absent"));
            }
            if state.failing_reads.contains(path) {
                return Err(
                    injected(ErrorKind::Unexpected, "injected: read failed").set_temporary()
                );
            }
        }
        self.inner.read_dyn(ctx, path, args)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<oio::Writer> {
        if self.state().failing_writes.contains(path) {
            return Err(injected(ErrorKind::Unexpected, "injected: write failed").set_temporary());
        }
        self.inner.write_dyn(ctx, path, args)
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
        opts: OpCopier,
    ) -> Result<oio::Copier> {
        self.inner.copy_dyn(ctx, from, to, args, opts)
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

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.inner.presign_dyn(ctx, path, args).await
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
