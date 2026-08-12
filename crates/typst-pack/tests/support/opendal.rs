use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::future::poll_fn;
use std::ops::Range;
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
    PublicationReadChunksExceeded { declared: usize, scripted: usize },
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationCapabilities {
    pub write: bool,
    pub write_can_empty: bool,
    pub write_with_if_not_exists: bool,
    pub read: bool,
    pub write_total_max_size: Option<usize>,
}

impl PublicationCapabilities {
    pub const fn all() -> Self {
        Self {
            write: true,
            write_can_empty: true,
            write_with_if_not_exists: true,
            read: true,
            write_total_max_size: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WriteCondition {
    Direct,
    IfNotExists,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteStage {
    Setup,
    Write,
    Close,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteEffect {
    NoEffect,
    Committed,
    Indeterminate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DestinationMutation {
    Set { path: String, bytes: Vec<u8> },
}

impl DestinationMutation {
    pub fn set(path: impl Into<String>, bytes: impl AsRef<[u8]>) -> Self {
        Self::Set {
            path: path.into(),
            bytes: bytes.as_ref().to_vec(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DestinationState {
    objects: BTreeMap<String, Vec<u8>>,
}

impl DestinationState {
    pub fn object(&self, path: &str) -> Option<&[u8]> {
        self.objects.get(path).map(Vec::as_slice)
    }
    fn apply(&mut self, mutation: &DestinationMutation) {
        match mutation {
            DestinationMutation::Set { path, bytes } => {
                self.objects.insert(path.clone(), bytes.clone());
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum PublicationReadStep {
    Chunk(Range<usize>),
    Pending(PendingPoint),
    Mutate(DestinationMutation),
    Failure(ErrorKind),
}

impl PublicationReadStep {
    pub fn chunk(range: Range<usize>) -> Self {
        Self::Chunk(range)
    }

    pub fn pending(point: PendingPoint) -> Self {
        Self::Pending(point)
    }

    pub fn mutate(mutation: DestinationMutation) -> Self {
        Self::Mutate(mutation)
    }

    pub const fn failure(kind: ErrorKind) -> Self {
        Self::Failure(kind)
    }
}

#[derive(Clone, Debug)]
pub struct PublicationReadScript {
    path: String,
    steps: Vec<PublicationReadStep>,
}

impl PublicationReadScript {
    pub fn new(
        path: impl Into<String>,
        declared_chunks: usize,
        steps: impl IntoIterator<Item = PublicationReadStep>,
    ) -> std::result::Result<Self, ScriptError> {
        let steps = steps.into_iter().collect::<Vec<_>>();
        let scripted = steps
            .iter()
            .filter(|step| matches!(step, PublicationReadStep::Chunk(_)))
            .count();
        if scripted > declared_chunks {
            return Err(ScriptError::PublicationReadChunksExceeded {
                declared: declared_chunks,
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
pub enum WriteStep {
    Pending(PendingPoint),
    Commit,
    Failure(ErrorKind),
}

impl WriteStep {
    pub fn pending(point: PendingPoint) -> Self {
        Self::Pending(point)
    }
    pub const fn commit() -> Self {
        Self::Commit
    }

    pub const fn failure(kind: ErrorKind) -> Self {
        Self::Failure(kind)
    }
}

#[derive(Clone, Debug)]
pub struct WriteScript {
    path: String,
    condition: WriteCondition,
    setup_failure: Option<ErrorKind>,
    write_failure: Option<ErrorKind>,
    close_steps: Vec<WriteStep>,
}

impl WriteScript {
    pub fn new(
        path: impl Into<String>,
        condition: WriteCondition,
        close_steps: impl IntoIterator<Item = WriteStep>,
    ) -> Self {
        Self {
            path: path.into(),
            condition,
            setup_failure: None,
            write_failure: None,
            close_steps: close_steps.into_iter().collect(),
        }
    }

    pub fn setup_failure(
        path: impl Into<String>,
        condition: WriteCondition,
        kind: ErrorKind,
    ) -> Self {
        let mut script = Self::new(path, condition, []);
        script.setup_failure = Some(kind);
        script
    }

    pub fn write_failure(
        path: impl Into<String>,
        condition: WriteCondition,
        kind: ErrorKind,
    ) -> Self {
        let mut script = Self::new(path, condition, []);
        script.write_failure = Some(kind);
        script
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicationOperationLogEntry {
    ReadInvoked {
        id: u64,
        path: String,
    },
    ReadChunkYielded {
        id: u64,
        path: String,
        bytes: Vec<u8>,
        destination: DestinationState,
    },
    ReadCompleted {
        id: u64,
        path: String,
        destination: DestinationState,
    },
    ReadFailed {
        id: u64,
        path: String,
        kind: ErrorKind,
        destination: DestinationState,
    },
    ReadDropped {
        id: u64,
        path: String,
        destination: DestinationState,
    },
    WriteInvoked {
        id: u64,
        path: String,
        condition: WriteCondition,
    },
    WriteAccepted {
        id: u64,
        path: String,
        length: usize,
        condition: WriteCondition,
    },
    WriteCompleted {
        id: u64,
        path: String,
        length: usize,
        condition: WriteCondition,
        effect: WriteEffect,
        destination: DestinationState,
    },
    WriteFailed {
        id: u64,
        path: String,
        length: usize,
        condition: WriteCondition,
        kind: ErrorKind,
        stage: WriteStage,
        issued: bool,
        effect: WriteEffect,
        destination: DestinationState,
    },
    WriteDropped {
        id: u64,
        path: String,
        length: usize,
        condition: WriteCondition,
        issued: bool,
        effect: WriteEffect,
        destination: DestinationState,
    },
    DestinationMutated {
        mutation: DestinationMutation,
        destination: DestinationState,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicationDroppedOperation {
    Read {
        id: u64,
        path: String,
    },
    Write {
        id: u64,
        path: String,
        length: usize,
        condition: WriteCondition,
        issued: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationOperationLog {
    entries: Vec<PublicationOperationLogEntry>,
    omitted_entries: usize,
}

impl PublicationOperationLog {
    pub fn entries(&self) -> &[PublicationOperationLogEntry] {
        &self.entries
    }

    pub const fn omitted_entries(&self) -> usize {
        self.omitted_entries
    }
}

#[derive(Clone)]
pub struct PublicationService {
    shared: Arc<PublicationShared>,
}

impl PublicationService {
    pub fn new(
        capabilities: PublicationCapabilities,
        initial_objects: impl IntoIterator<Item = (String, Vec<u8>)>,
        read_scripts: impl IntoIterator<Item = PublicationReadScript>,
        write_scripts: impl IntoIterator<Item = WriteScript>,
        log_capacity: usize,
    ) -> Self {
        let mut reads = BTreeMap::<_, VecDeque<_>>::new();
        for script in read_scripts {
            reads
                .entry(script.path.clone())
                .or_default()
                .push_back(script);
        }
        let mut writes = BTreeMap::<_, VecDeque<_>>::new();
        for script in write_scripts {
            writes
                .entry((script.path.clone(), script.condition))
                .or_default()
                .push_back(script);
        }

        Self {
            shared: Arc::new(PublicationShared {
                capabilities,
                destination: Mutex::new(DestinationState {
                    objects: initial_objects.into_iter().collect(),
                }),
                reads: Mutex::new(reads),
                writes: Mutex::new(writes),
                next_id: AtomicU64::new(0),
                log: Mutex::new(PublicationLogState {
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

    pub fn destination(&self) -> DestinationState {
        self.shared.destination()
    }

    pub fn mutate(&self, mutation: DestinationMutation) {
        self.shared.mutate(mutation);
    }

    pub fn log(&self) -> PublicationOperationLog {
        let log = lock(&self.shared.log);
        PublicationOperationLog {
            entries: log.entries.clone(),
            omitted_entries: log.omitted_entries,
        }
    }

    pub fn cancellations(&self) -> Vec<PublicationDroppedOperation> {
        lock(&self.shared.cancellations).clone()
    }
}

impl fmt::Debug for PublicationService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublicationService")
            .field("capabilities", &self.shared.capabilities)
            .field("destination", &self.destination())
            .finish_non_exhaustive()
    }
}

impl Service for PublicationService {
    type Reader = PublicationReader;
    type Writer = PublicationWriter;
    type Lister = ();
    type Deleter = ();
    type Copier = ();

    fn info(&self) -> ServiceInfo {
        ServiceInfo::with_scheme("scripted-publication-test")
    }

    fn capability(&self) -> Capability {
        Capability {
            write: self.shared.capabilities.write,
            write_can_empty: self.shared.capabilities.write_can_empty,
            write_with_if_not_exists: self.shared.capabilities.write_with_if_not_exists,
            read: self.shared.capabilities.read,
            write_total_max_size: self.shared.capabilities.write_total_max_size,
            ..Capability::default()
        }
    }

    async fn create_dir(
        &self,
        _: &OperationContext,
        _: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(publication_unsupported_operation())
    }

    async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
        Err(publication_unsupported_operation())
    }

    fn read(&self, _: &OperationContext, path: &str, _: OpRead) -> Result<Self::Reader> {
        let id = self.shared.next_id.fetch_add(1, Ordering::SeqCst);
        self.shared
            .record(PublicationOperationLogEntry::ReadInvoked {
                id,
                path: path.to_owned(),
            });
        let script = lock(&self.shared.reads)
            .get_mut(path)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                self.shared
                    .record(PublicationOperationLogEntry::ReadFailed {
                        id,
                        path: path.to_owned(),
                        kind: ErrorKind::NotFound,
                        destination: self.shared.destination(),
                    });
                publication_error(ErrorKind::NotFound, "no publication read script remains")
            })?;

        Ok(PublicationReader {
            script: Mutex::new(Some(script)),
            operation: Arc::new(PublicationOperationState::new_read(
                id,
                path,
                self.shared.clone(),
            )),
        })
    }

    fn write(&self, _: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let condition = if args.if_not_exists() {
            WriteCondition::IfNotExists
        } else {
            WriteCondition::Direct
        };
        let id = self.shared.next_id.fetch_add(1, Ordering::SeqCst);
        self.shared
            .record(PublicationOperationLogEntry::WriteInvoked {
                id,
                path: path.to_owned(),
                condition,
            });
        let mut script = lock(&self.shared.writes)
            .get_mut(&(path.to_owned(), condition))
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| {
                self.shared
                    .record(PublicationOperationLogEntry::WriteFailed {
                        id,
                        path: path.to_owned(),
                        length: 0,
                        condition,
                        kind: ErrorKind::Unexpected,
                        stage: WriteStage::Setup,
                        issued: false,
                        effect: WriteEffect::NoEffect,
                        destination: self.shared.destination(),
                    });
                publication_error(ErrorKind::Unexpected, "no publication write script remains")
            })?;
        if let Some(kind) = script.setup_failure.take() {
            self.shared
                .record(PublicationOperationLogEntry::WriteFailed {
                    id,
                    path: path.to_owned(),
                    length: 0,
                    condition,
                    kind,
                    stage: WriteStage::Setup,
                    issued: false,
                    effect: WriteEffect::NoEffect,
                    destination: self.shared.destination(),
                });
            return Err(publication_error(
                kind,
                "scripted publication setup failure",
            ));
        }

        Ok(PublicationWriter {
            write_failure: script.write_failure,
            close_steps: script.close_steps.into(),
            payload: None,
            committed: false,
            operation: Arc::new(PublicationOperationState::new_write(
                id,
                path,
                condition,
                self.shared.clone(),
            )),
        })
    }

    fn delete(&self, _: &OperationContext) -> Result<Self::Deleter> {
        Err(publication_unsupported_operation())
    }

    fn list(&self, _: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
        Err(publication_unsupported_operation())
    }

    fn copy(
        &self,
        _: &OperationContext,
        _: &str,
        _: &str,
        _: OpCopy,
        _: OpCopier,
    ) -> Result<Self::Copier> {
        Err(publication_unsupported_operation())
    }

    async fn rename(
        &self,
        _: &OperationContext,
        _: &str,
        _: &str,
        _: OpRename,
    ) -> Result<RpRename> {
        Err(publication_unsupported_operation())
    }

    async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
        Err(publication_unsupported_operation())
    }
}

struct PublicationShared {
    capabilities: PublicationCapabilities,
    destination: Mutex<DestinationState>,
    reads: Mutex<BTreeMap<String, VecDeque<PublicationReadScript>>>,
    writes: Mutex<BTreeMap<(String, WriteCondition), VecDeque<WriteScript>>>,
    next_id: AtomicU64,
    log: Mutex<PublicationLogState>,
    cancellations: Mutex<Vec<PublicationDroppedOperation>>,
}

impl PublicationShared {
    fn destination(&self) -> DestinationState {
        lock(&self.destination).clone()
    }

    fn mutate(&self, mutation: DestinationMutation) {
        let destination = {
            let mut destination = lock(&self.destination);
            destination.apply(&mutation);
            destination.clone()
        };
        self.record(PublicationOperationLogEntry::DestinationMutated {
            mutation,
            destination,
        });
    }

    fn record(&self, entry: PublicationOperationLogEntry) {
        let mut log = lock(&self.log);
        if log.entries.len() < log.capacity {
            log.entries.push(entry);
        } else {
            log.omitted_entries += 1;
        }
    }
}

struct PublicationLogState {
    capacity: usize,
    entries: Vec<PublicationOperationLogEntry>,
    omitted_entries: usize,
}

enum PublicationOperationKind {
    Read,
    Write {
        condition: WriteCondition,
        length: AtomicU64,
        issued: AtomicBool,
    },
}

struct PublicationOperationState {
    id: u64,
    path: String,
    kind: PublicationOperationKind,
    terminal: AtomicBool,
    shared: Arc<PublicationShared>,
}

impl PublicationOperationState {
    fn new_read(id: u64, path: &str, shared: Arc<PublicationShared>) -> Self {
        Self {
            id,
            path: path.to_owned(),
            kind: PublicationOperationKind::Read,
            terminal: AtomicBool::new(false),
            shared,
        }
    }

    fn new_write(
        id: u64,
        path: &str,
        condition: WriteCondition,
        shared: Arc<PublicationShared>,
    ) -> Self {
        Self {
            id,
            path: path.to_owned(),
            kind: PublicationOperationKind::Write {
                condition,
                length: AtomicU64::new(0),
                issued: AtomicBool::new(false),
            },
            terminal: AtomicBool::new(false),
            shared,
        }
    }

    fn accept_write(&self, length: usize) {
        let PublicationOperationKind::Write {
            condition,
            length: stored_length,
            issued,
        } = &self.kind
        else {
            unreachable!("only write operations accept bytes")
        };
        stored_length.store(length as u64, Ordering::SeqCst);
        issued.store(true, Ordering::SeqCst);
        self.shared
            .record(PublicationOperationLogEntry::WriteAccepted {
                id: self.id,
                path: self.path.clone(),
                length,
                condition: *condition,
            });
    }

    fn complete_read(&self) {
        if !self.terminal.swap(true, Ordering::SeqCst) {
            self.shared
                .record(PublicationOperationLogEntry::ReadCompleted {
                    id: self.id,
                    path: self.path.clone(),
                    destination: self.shared.destination(),
                });
        }
    }

    fn fail_read(&self, kind: ErrorKind) {
        if !self.terminal.swap(true, Ordering::SeqCst) {
            self.shared
                .record(PublicationOperationLogEntry::ReadFailed {
                    id: self.id,
                    path: self.path.clone(),
                    kind,
                    destination: self.shared.destination(),
                });
        }
    }

    fn complete_write(&self) {
        if self.terminal.swap(true, Ordering::SeqCst) {
            return;
        }
        let (condition, length, _) = self.write_state();
        self.shared
            .record(PublicationOperationLogEntry::WriteCompleted {
                id: self.id,
                path: self.path.clone(),
                length,
                condition,
                effect: WriteEffect::Committed,
                destination: self.shared.destination(),
            });
    }

    fn fail_write(&self, kind: ErrorKind, stage: WriteStage, effect: WriteEffect) {
        if self.terminal.swap(true, Ordering::SeqCst) {
            return;
        }
        let (condition, length, issued) = self.write_state();
        self.shared
            .record(PublicationOperationLogEntry::WriteFailed {
                id: self.id,
                path: self.path.clone(),
                length,
                condition,
                kind,
                stage,
                issued,
                effect,
                destination: self.shared.destination(),
            });
    }

    fn write_state(&self) -> (WriteCondition, usize, bool) {
        let PublicationOperationKind::Write {
            condition,
            length,
            issued,
        } = &self.kind
        else {
            unreachable!("only write operations expose write state")
        };
        (
            *condition,
            length.load(Ordering::SeqCst) as usize,
            issued.load(Ordering::SeqCst),
        )
    }
}

impl Drop for PublicationOperationState {
    fn drop(&mut self) {
        if self.terminal.swap(true, Ordering::SeqCst) {
            return;
        }
        match &self.kind {
            PublicationOperationKind::Read => {
                self.shared
                    .record(PublicationOperationLogEntry::ReadDropped {
                        id: self.id,
                        path: self.path.clone(),
                        destination: self.shared.destination(),
                    });
                lock(&self.shared.cancellations).push(PublicationDroppedOperation::Read {
                    id: self.id,
                    path: self.path.clone(),
                });
            }
            PublicationOperationKind::Write {
                condition,
                length,
                issued,
            } => {
                let length = length.load(Ordering::SeqCst) as usize;
                let issued = issued.load(Ordering::SeqCst);
                self.shared
                    .record(PublicationOperationLogEntry::WriteDropped {
                        id: self.id,
                        path: self.path.clone(),
                        length,
                        condition: *condition,
                        issued,
                        effect: if issued {
                            WriteEffect::Indeterminate
                        } else {
                            WriteEffect::NoEffect
                        },
                        destination: self.shared.destination(),
                    });
                lock(&self.shared.cancellations).push(PublicationDroppedOperation::Write {
                    id: self.id,
                    path: self.path.clone(),
                    length,
                    condition: *condition,
                    issued,
                });
            }
        }
    }
}

pub struct PublicationReader {
    script: Mutex<Option<PublicationReadScript>>,
    operation: Arc<PublicationOperationState>,
}

impl oio::Read for PublicationReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        if !range.is_full() {
            self.operation.fail_read(ErrorKind::Unsupported);
            return Err(publication_error(
                ErrorKind::Unsupported,
                "scripted publication reads require the full range",
            ));
        }
        let script = lock(&self.script).take().ok_or_else(|| {
            publication_error(
                ErrorKind::Unexpected,
                "publication read script already opened",
            )
        })?;
        let content_length = self
            .operation
            .shared
            .destination()
            .object(&self.operation.path)
            .map_or(0, |bytes| bytes.len() as u64);
        let metadata = Metadata::new(EntryMode::FILE).with_content_length(content_length);
        Ok((
            RpRead::new(metadata),
            Box::new(PublicationReadStream {
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

pub struct PublicationReadStream {
    steps: VecDeque<PublicationReadStep>,
    operation: Arc<PublicationOperationState>,
}

impl oio::ReadStream for PublicationReadStream {
    async fn read(&mut self) -> Result<Buffer> {
        loop {
            match self.steps.pop_front() {
                Some(PublicationReadStep::Chunk(range)) => {
                    let destination = self.operation.shared.destination();
                    let bytes = destination
                        .object(&self.operation.path)
                        .and_then(|bytes| bytes.get(range.clone()))
                        .ok_or_else(|| {
                            self.operation.fail_read(ErrorKind::RangeNotSatisfied);
                            publication_error(
                                ErrorKind::RangeNotSatisfied,
                                "publication read chunk is outside destination state",
                            )
                        })?
                        .to_vec();
                    self.operation
                        .shared
                        .record(PublicationOperationLogEntry::ReadChunkYielded {
                            id: self.operation.id,
                            path: self.operation.path.clone(),
                            bytes: bytes.clone(),
                            destination,
                        });
                    return Ok(Buffer::from(bytes));
                }
                Some(PublicationReadStep::Pending(point)) => point.wait().await,
                Some(PublicationReadStep::Mutate(mutation)) => {
                    self.operation.shared.mutate(mutation);
                }
                Some(PublicationReadStep::Failure(kind)) => {
                    self.operation.fail_read(kind);
                    return Err(publication_error(kind, "scripted publication read failure"));
                }
                None => {
                    self.operation.complete_read();
                    return Ok(Buffer::new());
                }
            }
        }
    }
}

pub struct PublicationWriter {
    write_failure: Option<ErrorKind>,
    close_steps: VecDeque<WriteStep>,
    payload: Option<Vec<u8>>,
    committed: bool,
    operation: Arc<PublicationOperationState>,
}

impl PublicationWriter {
    fn commit(&mut self) -> Result<()> {
        let payload = self.payload.as_ref().cloned().unwrap_or_default();
        let (condition, _, _) = self.operation.write_state();
        let mut destination = lock(&self.operation.shared.destination);
        if condition == WriteCondition::IfNotExists
            && destination.objects.contains_key(&self.operation.path)
        {
            drop(destination);
            self.operation.fail_write(
                ErrorKind::ConditionNotMatch,
                WriteStage::Close,
                WriteEffect::NoEffect,
            );
            return Err(publication_error(
                ErrorKind::ConditionNotMatch,
                "conditional publication destination exists",
            ));
        }
        destination
            .objects
            .insert(self.operation.path.clone(), payload);
        self.committed = true;
        Ok(())
    }
}

impl oio::Write for PublicationWriter {
    async fn write(&mut self, bytes: Buffer) -> Result<()> {
        let bytes = bytes.to_vec();
        self.operation.accept_write(bytes.len());
        if let Some(kind) = self.write_failure.take() {
            self.operation
                .fail_write(kind, WriteStage::Write, WriteEffect::Indeterminate);
            return Err(publication_error(
                kind,
                "scripted publication write failure",
            ));
        }
        if self.payload.replace(bytes).is_some() {
            self.operation.fail_write(
                ErrorKind::Unsupported,
                WriteStage::Write,
                WriteEffect::Indeterminate,
            );
            return Err(publication_error(
                ErrorKind::Unsupported,
                "scripted publication supports one write buffer",
            ));
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        if !self.operation.write_state().2 {
            self.operation.accept_write(0);
        }
        while let Some(step) = self.close_steps.pop_front() {
            match step {
                WriteStep::Pending(point) => point.wait().await,
                WriteStep::Commit => self.commit()?,
                WriteStep::Failure(kind) => {
                    self.operation
                        .fail_write(kind, WriteStage::Close, WriteEffect::Indeterminate);
                    return Err(publication_error(
                        kind,
                        "scripted publication close failure",
                    ));
                }
            }
        }
        if !self.committed {
            self.commit()?;
        }
        self.operation.complete_write();
        let length = self.operation.write_state().1 as u64;
        Ok(Metadata::new(EntryMode::FILE).with_content_length(length))
    }

    async fn abort(&mut self) -> Result<()> {
        self.payload = None;
        Ok(())
    }
}

fn publication_error(kind: ErrorKind, message: &'static str) -> Error {
    Error::new(kind, message).with_operation("scripted-publication-test")
}

fn publication_unsupported_operation() -> Error {
    publication_error(ErrorKind::Unsupported, "operation is not supported")
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
