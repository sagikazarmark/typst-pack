use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::future::poll_fn;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};

use opendal::raw::oio;
use opendal::raw::{
    OpCopier, OpCopy, OpCreateDir, OpList, OpPresign, OpRead, OpRename, OpStat, OpWrite,
    RpCreateDir, RpPresign, RpRead, RpRename, RpStat, Service, ServiceInfo,
};
use opendal::{
    Buffer, BytesRange, Capability, EntryMode, Error, ErrorKind, Metadata, OperationContext,
    Operator, Result,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub list: bool,
    pub list_with_recursive: bool,
    pub read: bool,
}

impl Capabilities {
    pub const fn all() -> Self {
        Self {
            list: true,
            list_with_recursive: true,
            read: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListEntryKind {
    File,
    Directory,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListEntry {
    path: String,
    kind: ListEntryKind,
}

impl ListEntry {
    pub fn file(path: impl Into<String>) -> Self {
        Self::new(path, ListEntryKind::File)
    }

    pub fn directory(path: impl Into<String>) -> Self {
        Self::new(path, ListEntryKind::Directory)
    }

    pub fn unknown(path: impl Into<String>) -> Self {
        Self::new(path, ListEntryKind::Unknown)
    }

    pub fn new(path: impl Into<String>, kind: ListEntryKind) -> Self {
        Self {
            path: path.into(),
            kind,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn kind(&self) -> ListEntryKind {
        self.kind
    }

    fn into_raw(self) -> oio::Entry {
        let mode = match self.kind {
            ListEntryKind::File => EntryMode::FILE,
            ListEntryKind::Directory => EntryMode::DIR,
            ListEntryKind::Unknown => EntryMode::Unknown,
        };
        oio::Entry::with(self.path, Metadata::new(mode))
    }
}

#[derive(Clone, Debug)]
pub enum ListStep {
    Page(Vec<ListEntry>),
    Pending(PendingPoint),
    Failure(ErrorKind),
}

impl ListStep {
    pub fn page(entries: impl IntoIterator<Item = ListEntry>) -> Self {
        Self::Page(entries.into_iter().collect())
    }

    pub fn pending(point: PendingPoint) -> Self {
        Self::Pending(point)
    }

    pub const fn failure(kind: ErrorKind) -> Self {
        Self::Failure(kind)
    }
}

#[derive(Clone, Debug)]
pub struct ListScript {
    path: String,
    steps: Vec<ListStep>,
}

impl ListScript {
    pub fn new(
        path: impl Into<String>,
        declared_entries: usize,
        steps: impl IntoIterator<Item = ListStep>,
    ) -> std::result::Result<Self, ScriptError> {
        let steps = steps.into_iter().collect::<Vec<_>>();
        let scripted = steps
            .iter()
            .map(|step| match step {
                ListStep::Page(entries) => entries.len(),
                ListStep::Pending(_) | ListStep::Failure(_) => 0,
            })
            .sum();
        if scripted > declared_entries {
            return Err(ScriptError::TooManyListEntries {
                declared: declared_entries,
                scripted,
            });
        }

        Ok(Self {
            path: path.into(),
            steps,
        })
    }
}

#[derive(Clone, Debug)]
pub enum ReadStep {
    Chunk(Vec<u8>),
    Pending(PendingPoint),
    Failure(ErrorKind),
}

impl ReadStep {
    pub fn chunk(bytes: impl AsRef<[u8]>) -> Self {
        Self::Chunk(bytes.as_ref().to_vec())
    }

    pub fn pending(point: PendingPoint) -> Self {
        Self::Pending(point)
    }

    pub const fn failure(kind: ErrorKind) -> Self {
        Self::Failure(kind)
    }
}

#[derive(Clone, Debug)]
pub struct ReadScript {
    path: String,
    steps: Vec<ReadStep>,
    content_length: u64,
}

impl ReadScript {
    pub fn new(
        path: impl Into<String>,
        declared_chunks: usize,
        steps: impl IntoIterator<Item = ReadStep>,
    ) -> std::result::Result<Self, ScriptError> {
        let steps = steps.into_iter().collect::<Vec<_>>();
        let chunks = steps
            .iter()
            .filter(|step| matches!(step, ReadStep::Chunk(_)))
            .count();
        if chunks > declared_chunks {
            return Err(ScriptError::TooManyReadChunks {
                declared: declared_chunks,
                scripted: chunks,
            });
        }
        let content_length = steps
            .iter()
            .filter_map(|step| match step {
                ReadStep::Chunk(bytes) => Some(bytes.len() as u64),
                ReadStep::Pending(_) | ReadStep::Failure(_) => None,
            })
            .sum();

        Ok(Self {
            path: path.into(),
            steps,
            content_length,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScriptError {
    TooManyListEntries { declared: usize, scripted: usize },
    TooManyReadChunks { declared: usize, scripted: usize },
}

#[derive(Clone, Default)]
pub struct PendingPoint {
    state: Arc<PendingState>,
}

impl PendingPoint {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn release(&self) {
        let waker = {
            let mut inner = lock(&self.state.inner);
            inner.released = true;
            inner.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    pub fn was_observed(&self) -> bool {
        self.state.observed.load(Ordering::SeqCst)
    }

    async fn wait(&self) {
        poll_fn(|cx| {
            self.state.observed.store(true, Ordering::SeqCst);
            let mut inner = lock(&self.state.inner);
            if inner.released {
                Poll::Ready(())
            } else {
                inner.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        })
        .await
    }
}

impl fmt::Debug for PendingPoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingPoint")
            .field("observed", &self.was_observed())
            .field("released", &lock(&self.state.inner).released)
            .finish()
    }
}

#[derive(Default)]
struct PendingState {
    observed: AtomicBool,
    inner: Mutex<PendingInner>,
}

#[derive(Default)]
struct PendingInner {
    released: bool,
    waker: Option<Waker>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationLogEntry {
    ListInvoked {
        id: u64,
        path: String,
        recursive: bool,
    },
    ListPageYielded {
        id: u64,
        entries: Vec<ListEntry>,
    },
    ListCompleted {
        id: u64,
    },
    ListFailed {
        id: u64,
        kind: ErrorKind,
    },
    ListDropped {
        id: u64,
        path: String,
    },
    ReadInvoked {
        id: u64,
        path: String,
    },
    ReadChunkYielded {
        id: u64,
        bytes: Vec<u8>,
    },
    ReadCompleted {
        id: u64,
    },
    ReadFailed {
        id: u64,
        kind: ErrorKind,
    },
    ReadDropped {
        id: u64,
        path: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DroppedOperation {
    List { id: u64, path: String },
    Read { id: u64, path: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationLog {
    entries: Vec<OperationLogEntry>,
    omitted_entries: usize,
}

impl OperationLog {
    pub fn entries(&self) -> &[OperationLogEntry] {
        &self.entries
    }

    pub const fn omitted_entries(&self) -> usize {
        self.omitted_entries
    }
}

#[derive(Clone)]
pub struct ScriptedService {
    shared: Arc<Shared>,
}

impl ScriptedService {
    pub fn new(
        capabilities: Capabilities,
        list_scripts: impl IntoIterator<Item = ListScript>,
        read_scripts: impl IntoIterator<Item = ReadScript>,
        log_capacity: usize,
    ) -> Self {
        let mut lists = BTreeMap::<_, VecDeque<_>>::new();
        for script in list_scripts {
            lists
                .entry(script.path.clone())
                .or_default()
                .push_back(script);
        }
        let mut reads = BTreeMap::<_, VecDeque<_>>::new();
        for script in read_scripts {
            reads
                .entry(script.path.clone())
                .or_default()
                .push_back(script);
        }

        Self {
            shared: Arc::new(Shared {
                capabilities,
                lists: Mutex::new(lists),
                reads: Mutex::new(reads),
                next_id: AtomicU64::new(0),
                log: Mutex::new(LogState {
                    capacity: log_capacity,
                    entries: Vec::with_capacity(log_capacity),
                    omitted_entries: 0,
                }),
                cancellations: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn operator(&self) -> Operator {
        Operator::from_parts(OperationContext::default(), Arc::new(self.clone()))
    }

    pub fn log(&self) -> OperationLog {
        let log = lock(&self.shared.log);
        OperationLog {
            entries: log.entries.clone(),
            omitted_entries: log.omitted_entries,
        }
    }

    pub fn cancellations(&self) -> Vec<DroppedOperation> {
        lock(&self.shared.cancellations).clone()
    }
}

impl fmt::Debug for ScriptedService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScriptedService")
            .field("capabilities", &self.shared.capabilities)
            .finish_non_exhaustive()
    }
}

impl Service for ScriptedService {
    type Reader = ScriptedReader;
    type Writer = ();
    type Lister = ScriptedLister;
    type Deleter = ();
    type Copier = ();

    fn info(&self) -> ServiceInfo {
        ServiceInfo::with_scheme("scripted-test")
    }

    fn capability(&self) -> Capability {
        Capability {
            list: self.shared.capabilities.list,
            list_with_recursive: self.shared.capabilities.list_with_recursive,
            read: self.shared.capabilities.read,
            ..Capability::default()
        }
    }

    async fn create_dir(
        &self,
        _: &OperationContext,
        _: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(unsupported_operation())
    }

    async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
        Err(unsupported_operation())
    }

    fn read(&self, _: &OperationContext, path: &str, _: OpRead) -> Result<Self::Reader> {
        let id = self.shared.next_id.fetch_add(1, Ordering::SeqCst);
        self.shared.record(OperationLogEntry::ReadInvoked {
            id,
            path: path.to_owned(),
        });
        let script = lock(&self.shared.reads)
            .get_mut(path)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                self.shared.record(OperationLogEntry::ReadFailed {
                    id,
                    kind: ErrorKind::NotFound,
                });
                scripted_error(ErrorKind::NotFound, "no read script remains")
            })?;

        Ok(ScriptedReader {
            script: Mutex::new(Some(script)),
            operation: Arc::new(OperationState::new(
                id,
                path,
                OperationKind::Read,
                self.shared.clone(),
            )),
        })
    }

    fn write(&self, _: &OperationContext, _: &str, _: OpWrite) -> Result<Self::Writer> {
        Err(unsupported_operation())
    }

    fn delete(&self, _: &OperationContext) -> Result<Self::Deleter> {
        Err(unsupported_operation())
    }

    fn list(&self, _: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let id = self.shared.next_id.fetch_add(1, Ordering::SeqCst);
        self.shared.record(OperationLogEntry::ListInvoked {
            id,
            path: path.to_owned(),
            recursive: args.recursive(),
        });
        let script = lock(&self.shared.lists)
            .get_mut(path)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                self.shared.record(OperationLogEntry::ListFailed {
                    id,
                    kind: ErrorKind::NotFound,
                });
                scripted_error(ErrorKind::NotFound, "no list script remains")
            })?;

        Ok(ScriptedLister {
            steps: script.steps.into(),
            page: None,
            operation: Arc::new(OperationState::new(
                id,
                path,
                OperationKind::List,
                self.shared.clone(),
            )),
        })
    }

    fn copy(
        &self,
        _: &OperationContext,
        _: &str,
        _: &str,
        _: OpCopy,
        _: OpCopier,
    ) -> Result<Self::Copier> {
        Err(unsupported_operation())
    }

    async fn rename(
        &self,
        _: &OperationContext,
        _: &str,
        _: &str,
        _: OpRename,
    ) -> Result<RpRename> {
        Err(unsupported_operation())
    }

    async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
        Err(unsupported_operation())
    }
}

struct Shared {
    capabilities: Capabilities,
    lists: Mutex<BTreeMap<String, VecDeque<ListScript>>>,
    reads: Mutex<BTreeMap<String, VecDeque<ReadScript>>>,
    next_id: AtomicU64,
    log: Mutex<LogState>,
    cancellations: Mutex<Vec<DroppedOperation>>,
}

impl Shared {
    fn record(&self, entry: OperationLogEntry) {
        let mut log = lock(&self.log);
        if log.entries.len() < log.capacity {
            log.entries.push(entry);
        } else {
            log.omitted_entries += 1;
        }
    }
}

struct LogState {
    capacity: usize,
    entries: Vec<OperationLogEntry>,
    omitted_entries: usize,
}

#[derive(Clone, Copy)]
enum OperationKind {
    List,
    Read,
}

struct OperationState {
    id: u64,
    path: String,
    kind: OperationKind,
    terminal: AtomicBool,
    shared: Arc<Shared>,
}

impl OperationState {
    fn new(id: u64, path: &str, kind: OperationKind, shared: Arc<Shared>) -> Self {
        Self {
            id,
            path: path.to_owned(),
            kind,
            terminal: AtomicBool::new(false),
            shared,
        }
    }

    fn complete(&self) {
        if !self.terminal.swap(true, Ordering::SeqCst) {
            self.shared.record(match self.kind {
                OperationKind::List => OperationLogEntry::ListCompleted { id: self.id },
                OperationKind::Read => OperationLogEntry::ReadCompleted { id: self.id },
            });
        }
    }

    fn fail(&self, kind: ErrorKind) {
        if !self.terminal.swap(true, Ordering::SeqCst) {
            self.shared.record(match self.kind {
                OperationKind::List => OperationLogEntry::ListFailed { id: self.id, kind },
                OperationKind::Read => OperationLogEntry::ReadFailed { id: self.id, kind },
            });
        }
    }
}

impl Drop for OperationState {
    fn drop(&mut self) {
        if self.terminal.swap(true, Ordering::SeqCst) {
            return;
        }
        let dropped = match self.kind {
            OperationKind::List => DroppedOperation::List {
                id: self.id,
                path: self.path.clone(),
            },
            OperationKind::Read => DroppedOperation::Read {
                id: self.id,
                path: self.path.clone(),
            },
        };
        self.shared.record(match self.kind {
            OperationKind::List => OperationLogEntry::ListDropped {
                id: self.id,
                path: self.path.clone(),
            },
            OperationKind::Read => OperationLogEntry::ReadDropped {
                id: self.id,
                path: self.path.clone(),
            },
        });
        lock(&self.shared.cancellations).push(dropped);
    }
}

pub struct ScriptedLister {
    steps: VecDeque<ListStep>,
    page: Option<ListPage>,
    operation: Arc<OperationState>,
}

struct ListPage {
    entries: Vec<ListEntry>,
    remaining: VecDeque<oio::Entry>,
}

impl oio::List for ScriptedLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        loop {
            if let Some(page) = &mut self.page {
                if let Some(entry) = page.remaining.pop_front() {
                    if page.remaining.is_empty() {
                        self.operation
                            .shared
                            .record(OperationLogEntry::ListPageYielded {
                                id: self.operation.id,
                                entries: page.entries.clone(),
                            });
                        self.page = None;
                    }
                    return Ok(Some(entry));
                }
            }
            match self.steps.pop_front() {
                Some(ListStep::Page(entries)) => {
                    let remaining = entries.iter().cloned().map(ListEntry::into_raw).collect();
                    self.page = Some(ListPage { entries, remaining });
                }
                Some(ListStep::Pending(point)) => point.wait().await,
                Some(ListStep::Failure(kind)) => {
                    self.operation.fail(kind);
                    return Err(scripted_error(kind, "scripted list failure"));
                }
                None => {
                    self.operation.complete();
                    return Ok(None);
                }
            }
        }
    }
}

pub struct ScriptedReader {
    script: Mutex<Option<ReadScript>>,
    operation: Arc<OperationState>,
}

impl oio::Read for ScriptedReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        if !range.is_full() {
            self.operation.fail(ErrorKind::Unsupported);
            return Err(scripted_error(
                ErrorKind::Unsupported,
                "scripted reads require the full range",
            ));
        }
        let script = lock(&self.script)
            .take()
            .ok_or_else(|| scripted_error(ErrorKind::Unexpected, "read script already opened"))?;
        let metadata = Metadata::new(EntryMode::FILE).with_content_length(script.content_length);
        Ok((
            RpRead::new(metadata),
            Box::new(ScriptedReadStream {
                steps: script.steps.into(),
                operation: self.operation.clone(),
            }),
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let (response, mut stream) = self.open(range).await?;
        let mut chunks = Vec::new();
        loop {
            let chunk = stream.read_dyn().await?;
            if chunk.is_empty() {
                break;
            }
            chunks.push(chunk);
        }
        Ok((response, chunks.into_iter().flatten().collect()))
    }
}

pub struct ScriptedReadStream {
    steps: VecDeque<ReadStep>,
    operation: Arc<OperationState>,
}

impl oio::ReadStream for ScriptedReadStream {
    async fn read(&mut self) -> Result<Buffer> {
        loop {
            match self.steps.pop_front() {
                Some(ReadStep::Chunk(bytes)) => {
                    self.operation
                        .shared
                        .record(OperationLogEntry::ReadChunkYielded {
                            id: self.operation.id,
                            bytes: bytes.clone(),
                        });
                    return Ok(Buffer::from(bytes));
                }
                Some(ReadStep::Pending(point)) => point.wait().await,
                Some(ReadStep::Failure(kind)) => {
                    self.operation.fail(kind);
                    return Err(scripted_error(kind, "scripted read failure"));
                }
                None => {
                    self.operation.complete();
                    return Ok(Buffer::new());
                }
            }
        }
    }
}

fn scripted_error(kind: ErrorKind, message: &'static str) -> Error {
    Error::new(kind, message).with_operation("scripted-test")
}

fn unsupported_operation() -> Error {
    scripted_error(ErrorKind::Unsupported, "operation is not supported")
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
