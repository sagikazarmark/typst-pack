# Typst Pack

This context describes portable Typst projects and the values that vary between individual document renders.

## Language

**Pack**:
A portable, reusable Typst compilation closure with one fixed entrypoint, established by successful Discovery Variants. It contains every observed or explicitly included project file, identifies each complete package and exact font dependency, and permits no undeclared dependency fallback.

**Discovery Variant**:
One member of a nonempty, explicitly ordered creation list, containing one exact representative compilation request: target, Typst inputs, time, enabled features, and any discovery-only project overrides. Its canonical identity is derived from that semantic request and its commitments; an optional public label and declaration order aid reporting without changing coverage identity. Canonical identities must be unique within one creation. A Pack's completeness guarantee covers the union of its recorded successful Discovery Variants and Explicit Conditional Inclusions, not arbitrary future variation; variants document coverage rather than restrict later compilation requests.

**Discovery World**:
The fixed logical project tree, entrypoint, Typst engine configuration, and package and font authorities against which every Discovery Variant in one Pack creation runs. Environment variables, wall-clock time, host fonts, caches, and other adapter defaults are resolved into this world or exact variant request values before semantic creation begins. Materially different Discovery Worlds produce separate Packs rather than variants of one Pack.

**Discovery Snapshot**:
The stable project, package, and font bytes, relevant directory membership, logical absences, and exact request values retained during one Pack creation. A mutable adapter must detect changes to consumed content or successful missing probes before issuance rather than silently create a Pack from mixed-time reads. Assembled-Pack replay uses this frozen snapshot without reacquisition; sensitive raw request values are discarded after the attempt.

**Discovery Trace**:
The canonical per-variant observation set of successful project and package reads, exact used font faces, and logical missing probes, including authority, request outcome, and effective baseline or override provenance. It is the persisted portable semantic projection of the same observation schema used by a Compilation Access Trace; source-specific Dependency Evidence Keys remain operation state. Access order and repeat counts are not semantic. A Pack's compilation closure is derived from the union of its Discovery Traces and Explicit Conditional Inclusions. The trace is persisted as discovery evidence; rendered compiler warnings and diagnostic text are not Pack state.

**Explicit Conditional Inclusion**:
An existing root-relative project file and its baseline bytes added to a Pack's compilation closure without being successfully read by a Discovery Variant. Directory and glob inputs are adapter conveniences expanded into this exact file set during creation. There is no implicit third project-file provenance category.

**Discovery Request Commitment**:
A canonical digest that identifies a potentially sensitive Discovery Variant input value or discovery-only override without storing the raw value. A commitment supports exact matching when the value is supplied again, but does not claim confidentiality against guessing.

**Pack Issuance**:
The point at which semantic creation returns a Pack after every Discovery Variant succeeds, the Discovery Snapshot is revalidated, whole-Pack invariants hold, and assembled-Pack replay reproduces every Discovery Trace. Archive or transport writing happens after issuance and commits atomically where its sink permits.

**Creation Report**:
The transient diagnostics from a Pack creation attempt, presented deterministically by validation, discovery, snapshot and assembly, replay, and issuance phase. Discovery and replay diagnostics follow Discovery Variant declaration order and identify the variant without making rendered messages part of the Pack.

**Pack Identity**:
The content identity of a Pack's canonical logical compilation state: its fixed entrypoint, discovery coverage identities, contained project files, ordered Pack Font Catalog, dependency requirements, and whether each dependency is embedded or externally fulfilled. Non-identifying provenance, archive encoding, and source-host filesystem metadata do not affect it.

**Portable Pack**:
A Pack that can move between hosts and compile when its exact declared external package and font bytes are supplied, without consulting the source host or undeclared ambient state. Every valid Pack provides this guarantee.

**Self-Contained Pack**:
A Pack that embeds every package and font dependency. Compiling one requires only the Pack, a compatible Typst engine, and explicit compilation request values; it does not consult the host filesystem, environment, caches, installed fonts, or network.

**Pack Manifest**:
The versioned declarative description carried by a Pack. It describes intended contents and metadata, while agreement between those declarations and contained files is a Pack invariant.

**Pack Inspection**:
The side-effect-free structured description of a validated Pack's complete static contract. It does not acquire external dependencies, compile, apply Pack Overrides, or expose archive transport details.

**Project Materialization**:
The exact projection of a Pack's baseline project paths and bytes into a fresh editable project tree. It excludes Pack metadata, packages, fonts, and Pack Overrides, and does not preserve Pack Identity when used as the input to a new Pack creation.

**Closure Export**:
A versioned, deterministic, namespaced filesystem representation of all semantic Pack state. It can reconstruct the same logical Pack and Pack Identity, represents externally fulfilled package and font requirements without acquiring their bytes, and excludes archive transport and host filesystem metadata.

**Package Requirement**:
A Pack dependency identified by both its exact Typst package specification and the content identity of its complete logical package tree. Acquisition authority and source location are provenance rather than identity. Identical content under different package specifications remains distinct, while different content supplied for one specification is an integrity conflict.

**Complete Package Tree**:
Every addressable regular file beneath an acquired package root, represented by its canonical package-relative path and bytes. It includes files not read during discovery and package metadata, but excludes empty directories, archive encoding, and source-host filesystem metadata.

**Package Authority**:
An explicit source of Complete Package Trees for exact Typst package specifications. It owns namespace support, source routing, fallback, acquisition, and offline policy, and reports acquisition provenance. Semantic Pack operations never consult ambient package locations outside their supplied Package Authority.

**Package Acquisition Provenance**:
A sanitized, non-identifying summary of the authority and source class from which a Complete Package Tree was acquired. It excludes credentials, transient attempts, and absolute host paths, and does not contribute to Package Requirement or Pack Identity.

**Offline Package Authority**:
A Package Authority that performs no network acquisition. It may still serve explicitly supplied, in-memory, configured local, or previously cached Complete Package Trees.

**External Package Fulfillment**:
Supplying a non-embedded Package Requirement through the compilation's explicit Package Authority. Fulfillment is directed by the requirement's logical and content identities rather than a serialized source location, and the supplied Complete Package Tree is independently verified before use.

**Package Embedding Policy**:
A deterministic choice, made independently for each Package Requirement after discovery, to embed its Complete Package Tree or require External Package Fulfillment. The choices affect Pack Identity and determine whether the Pack is self-contained.

**Font Container**:
The exact bytes of one standalone font file or multi-face font collection. Its canonical content identity is independent of source location, and all faces in a collection travel as one container.

**Font Face Identity**:
An exact face within a Font Container, identified by the container's content identity and the face's container-local index. Names, style, coverage, and other face metadata are derived from the verified container rather than independent identity.

**Font Requirement**:
A Pack dependency consisting of one Font Container identity and the nonempty set of Font Face Identities observed across Discovery Variants. Identical container bytes acquired from different locations form one requirement.

**Font Catalog Snapshot**:
The finite, explicitly ordered set of candidate face metadata and stable Font Container acquisition identities fixed for one Discovery World. An adapter prepares it from intentional sources before semantic discovery; selected container bytes are frozen on first use rather than every candidate being read eagerly.

**Font Authority**:
An explicit source of a Font Catalog Snapshot for discovery and exact Font Containers for external fulfillment. It owns source composition and provenance, while Typst selects faces from the supplied catalog.

**Offline Font Authority**:
A Font Authority that performs no network acquisition. It may still serve explicitly supplied, in-memory, configured local, system, engine-embedded, or previously cached Font Containers.

**Font Acquisition Provenance**:
A sanitized, non-identifying summary of the authority and source class from which a Font Container was acquired. It excludes credentials, transient attempts, and absolute host paths, and does not contribute to Font Requirement or Pack Identity.

**Font Scan Policy**:
The declared rules a Font Authority uses to omit, warn about, or reject invalid and unreadable candidates while constructing a Font Catalog Snapshot. The policy is fixed within one Discovery World, and its diagnostics are never silently discarded.

**Font Licensing Metadata**:
Advisory standardized licensing and embedding fields derived from a verified Font Container for policy and reporting. They do not establish legal permission to redistribute or embed the font.

**Pack Font Catalog**:
The ordered projection of a Discovery World's Font Catalog Snapshot containing exactly the Font Face Identities required by a Pack. It preserves their relative discovery order; other faces physically present in a required Font Container remain unavailable to compilation.

**Font Embedding Policy**:
A deterministic choice, made independently for each Font Requirement after discovery, to embed its Font Container or require External Font Fulfillment. The policy may inspect Font Licensing Metadata, and its choices affect Pack Identity and self-containment.

**External Font Fulfillment**:
Supplying a non-embedded Font Requirement through the compilation's explicit Font Authority. Fulfillment is directed by the requirement's exact identity rather than creation-time source location or fresh family and style selection.

**Document Format**:
A compilation format that produces one Compilation Output Artifact for a selected or unpaged document. PDF and HTML are Document Formats.

**Page Format**:
A compilation format that produces one Compilation Output Artifact for each selected source page. PNG and SVG are Page Formats; merged raster and SVG rendering are not formats. Page selection is a canonical set: each selected Source Page Number appears at most once, and artifacts are ordered by Source Page Number rather than declaration or exporter completion order.

**Source Page Number**:
The one-based physical position of a page in the source document before page selection, distinct from its emission order or printed page label.

**Compilation Output Artifact**:
One exact owned byte value produced by compiling a Pack. Its semantic role carries its Document Format or Page Format and, for a Page Format, one Source Page Number. Filenames, destinations, storage handles, and transport metadata are not part of the artifact.
_Avoid_: Output buffer, result file

**Compilation Artifact Identity**:
The identity of one Compilation Output Artifact: its artifact role within one Compilation Identity together with the content identity of its exact emitted bytes. The role contains the output format and, for a Page Format, the Source Page Number. Output destinations and transport encoding are not part of it.

**Compilation Document Time**:
The exact or explicitly absent semantic time used to answer Typst document-time requests such as `datetime.today()`. The core default is absent; an adapter must resolve any wall-clock or environment default before preparation. It is distinct from PDF Creation Time and contributes to Compilation Identity whether or not document code observes it.

**PDF Creation Time**:
The exact or explicitly omitted timestamp offered to PDF creation metadata when source metadata does not override it. The core default is omitted and is never implicitly derived from Compilation Document Time. It is a PDF-specific semantic output control, distinct from Compilation Document Time, and contributes to Compilation Identity. An adapter may intentionally resolve one external value into both times.

**Compilation Output Specification**:
The required tagged semantic output request whose variant determines both format and Typst target. PDF carries page selection, identifier and creator modes, PDF Creation Time, standards, tagging, and pretty-printing; PNG carries page selection, finite positive pixels per inch, and bleed rendering; SVG carries page selection, bleed rendering, and pretty-printing; HTML carries pretty-printing and core-derives the required HTML engine feature. Core defaults follow the embedded engine's deterministic defaults: all pages, 144 pixels per inch, automatic PDF identifier and creator, tagging enabled, and pretty-printing and bleed disabled. A PDF page subset with default tagging derives tagging disabled and a canonical warning; explicit tagging with ranges, a tag-required standard without tags, or an incompatible standard set is a Compilation Request Rejection.

**Compilation Request Inventory**:
The canonical description of every value supplied to or deterministically resolved for one compilation, including values that Typst does not observe. It records whether an effective value was caller-supplied, core-defaulted, core-derived, or adapter-resolved and marks each entry as semantic or operational. Semantic entries contribute to Compilation Identity, while authorities, caches, retries, deadlines, isolation, transport, output destinations, filename templates, terminal rendering, and other operational controls do not unless they resolve an explicit semantic value. A Prepared Compilation owns the semantic portion; each execution report adds the operational controls used for that attempt. Potentially sensitive values are represented publicly by Compilation Request Commitments rather than raw bytes. It is distinct from resolution evidence and observed access.

**Compilation Request Commitment**:
A canonical domain-separated digest that binds a potentially sensitive compilation input value or Pack Override without exposing its raw bytes. The raw value lives only in the active request unless a caller explicitly requests sensitive adapter output; the commitment and size can appear in public inventories and identities.

**Prepared Compilation**:
An immutable, Pack-bound compilation value produced before authority access or Typst execution by requiring a Compilation Output Specification, applying core-owned deterministic defaults, deriving the Typst target and required format features, canonicalizing and validating the complete semantic request, performing strict Pack Override preflight, and attesting the core's actual engine and exporter identity. The other core defaults are an empty Typst input map and caller-selected feature set and absent Compilation Document Time. HTML derives its required feature; the accessibility feature may be selected; the unsupported bundle feature is a Compilation Request Rejection. It owns the fully explicit semantic request, the semantic portion of the Compilation Request Inventory, and the Compilation Identity. It may be executed repeatedly or concurrently; attempts share only facilities explicitly supplied through their independent Compilation Execution Controls.

**Synchronous Compilation Driver**:
The public execution mode that runs a full Compilation Attempt on its caller's thread. It resolves exact requirements through explicit synchronous authorities and caches, constructs a Compilation Dependency Snapshot, and invokes the featureless synchronous Compilation Kernel. It produces the same Compilation Report model as the Asynchronous Compilation Driver and introduces no hidden async runtime, filesystem, network, or process requirement.

**Asynchronous Compilation Driver**:
The public execution mode that runs the same full Compilation Attempt lifecycle through asynchronous authorities and caches, constructs a Compilation Dependency Snapshot, and dispatches the same Compilation Kernel through an explicit Compilation Execution Facility. Its orchestration and transport behavior do not create a second semantic compilation path.

**Compilation Kernel**:
The single synchronous semantic path shared by the Synchronous Compilation Driver and Asynchronous Compilation Driver from accepted exact dependencies through Typst compilation and complete export. It has no asynchronous interface or ambient I/O and preserves one Compilation Report contract across drivers.

**Compilation Request Rejection**:
The deterministic, side-effect-free rejection of a semantic request before a Prepared Compilation or Compilation Identity exists. It retains the supplied semantic request inventory and every independently detectable typed issue in stable order, but has no dependency evidence or access trace.

**Compilation Identity**:
The pre-execution identity of one fully specified semantic compilation request. It binds the Pack Identity and every identity-bearing request value, including the Pack Override Set, the canonical Typst input map and feature set, Compilation Document Time, engine and exporter, output format and its derived target, page selection, PDF Creation Time when applicable, and every other byte- or Canonical Compilation Diagnostic-affecting output option, whether or not later execution observes that value. Output destinations, filename templates, color, timings, sink behavior, backing source, and acquisition provenance are not part of it. Adapters resolve every ambient output-affecting default into an explicit semantic value before computing the identity; hidden ambient fallback is forbidden.

**Compilation Result Identity**:
The immutable post-execution identity of one completed compiler and exporter result after exact semantic inputs are accepted and exact dependencies are available. It binds the Compilation Identity to status, Compilation Document Summary, Canonical Compilation Diagnostics, the semantic logical/content/outcome projection of dependency observations, and every ordered Compilation Artifact Identity. Backing source keys, cache hits, acquisition provenance, timing telemetry, Diagnostic Source Bundles, and Compilation Operation Outcomes do not contribute. Later changes may supersede its currentness but do not invalidate the historical value.

**Compilation Result**:
The immutable semantic terminal value produced after exact semantic inputs are accepted and exact dependencies are available. A successful Document Format result owns exactly one Compilation Output Artifact. A successful Page Format result owns one artifact per selected source page in Source Page Number order and may own none when a valid selection matches no pages. A rejected compiler or exporter result owns no artifacts. Both statuses retain all Canonical Compilation Diagnostics, a Compilation Document Summary, the canonical Compilation Access Trace and semantic dependency projection produced by that engine attempt, and a Compilation Result Identity. Warnings never change status or remove artifacts. A semantic cache returns this same value without replacing its original trace with current cache telemetry.

**Compilation Document Summary**:
The minimal typed description of the document reached by semantic compilation. It always identifies the derived target and records the total Source Page Number count when a paged document was produced, including when later export rejects. It does not expose or retain the upstream Typst document.

**Compilation Report**:
The owned immutable account of one attempt to execute a Prepared Compilation through complete export, before artifact publication. It contains the Compilation Request Inventory complete through that seam, current-attempt Dependency Resolution Evidence and cache provenance, the terminal phase and explicit reached scope of each dependency view, and exactly one terminal Compilation Result or Compilation Operation Outcome. When no semantic result exists, it also retains the partial Compilation Access Trace reached by that attempt; otherwise the result owns the canonical trace. Operational reporting channels may add acquisition, compilation, and export timing telemetry or a sensitive Diagnostic Source Bundle; each channel reports whether it was not requested, complete, limited, or unavailable without changing the terminal semantic result. Adapters wrap this report with their additional inventory and delivery outcomes rather than mutating it or replacing its semantic result.

**Compilation Execution Controls**:
The immutable operational configuration for one Compilation Attempt: explicit synchronous or asynchronous package and font authorities, cache policy, Compilation Resource Limits, Compilation Attempt Deadline and cancellation, monotonic time domain, concurrency or isolation facilities, spooling policy where applicable, and reporting policy. An absent cache, deadline, cancellation source, or telemetry request is explicit rather than ambient. Effects and sharing occur only through these named controls; they do not contribute to Compilation Identity unless they resolve an explicit semantic value.

**Compilation Resource Limits**:
The operational limits applied at the seam that can observe and enforce them. Drivers enforce dependency counts and declared sizes, downloaded and expanded bytes, spooling and cache budgets, and acquisition concurrency; the Compilation Kernel enforces observable logical and artifact limits; Compilation Delivery enforces transport and sink limits. In-process retained-memory and CPU limits are best-effort because Typst is non-cooperative. Only an Isolated Compilation Worker with enforced quotas may claim hard whole-attempt memory, CPU, or forced-termination bounds. Concrete budgets are adapter policy rather than semantic identity.

**Compilation Attempt**:
One execution of a Prepared Compilation under one Compilation Execution Controls snapshot, from admission through Compilation Terminal Commitment, producing one Compilation Report. A Compilation Driver never absorbs a terminal retry into the same attempt: a caller, session, or adapter retry produces a fresh report. Only bounded source-specific transport retries inside one authority acquisition remain internal operational telemetry.

**Compilation Execution Facility**:
An explicit caller-supplied facility used by the Asynchronous Compilation Driver for bounded admission, queueing, worker concurrency, and in-process blocking or isolated dispatch. It may be intentionally shared across attempts but owns no semantic state; queue time consumes each attempt's Compilation Attempt Deadline, and fairness and completion order are not semantic. The Synchronous Compilation Driver instead runs on its caller's thread, and typst-pack owns no global scheduler.

**Isolated Compilation Worker**:
An optional process behind a versioned Compilation Execution Facility protocol that receives only a Prepared Compilation, a verified read-only Compilation Dependency Snapshot or confined handles to it, resource controls, and the exact engine and exporter implementation. Acquisition, credentials, mutable caches, and asynchronous I/O remain in the parent. The parent exposes no output before it verifies a complete report, never silently falls back in-process, and reports worker crash, protocol, and limit failures as isolation outcomes. Isolation guarantees killability and resource placement, not hostile-input sandboxing by itself.

**Compilation Dependency Snapshot**:
The immutable, verified set of exact dependency bindings handed to the Compilation Kernel for one attempt. Every dependency is synchronously readable from stable memory or local backing; a whole file or Font Container may be loaded lazily, but remote acquisition, asynchronous waiting, mutable-source lookup, and runtime bridging are forbidden inside the kernel. Before this snapshot is complete, a Compilation Driver acquires and verifies every required external Package Requirement and Font Requirement, boundedly spools non-seekable inputs, and performs a bounded collect-all sweep of independent requirements unless interruption or resource limits intervene. Acquisition outcomes are reported in canonical requirement order rather than completion order.

**Compilation Interruption Strength**:
The reported operational guarantee used for cancellation and deadlines during one attempt. Cooperative in-process interruption stops controllable driver phases and suppresses a result when interruption wins before terminal commitment, but waits for a running non-cooperative Compilation Kernel to unwind. Isolated interruption may forcibly terminate and reap its worker. Neither strength permits hidden attempt work to survive the driver's return.

**Compilation Attempt Deadline**:
One optional absolute monotonic deadline governing an execution attempt from admission and queueing through acquisition, verification, spooling, Compilation Kernel execution and export, and report finalization at Compilation Terminal Commitment. Phase budgets may shorten but never extend it. Artifact publication, result delivery, and caller-requested persistence are later adapter operations with separate deadlines and outcomes.

**Compilation Terminal Commitment**:
The linearization point after the Compilation Kernel and report finalization complete but before a terminal value is exposed. Cancellation, deadline, or supersession recorded before this point wins and produces its Compilation Operation Outcome; interruption recorded afterward cannot mutate the committed Compilation Result.

**Compilation Delivery**:
An adapter operation after Compilation Terminal Commitment that transports or durably persists an immutable Compilation Report and its exact artifact bytes. It may stream with backpressure and commit atomically where its sink permits, but it never changes the report or triggers recompilation.

**Compilation Delivery Outcome**:
The typed operational result of one Compilation Delivery, including deadline, cancellation, partial-transfer cleanup, sink, and persistence failures. It refers to the immutable Compilation Report it attempted to deliver and never replaces that report's Compilation Result or Compilation Operation Outcome.

**Compilation Operation Outcome**:
The typed non-semantic outcome of running a Prepared Compilation before Compilation Terminal Commitment. Its closed classes cover dependency resolution and acquisition, dynamic resource limits, deadline, cancellation, supersession, execution or isolation failure, and internal integrity failure, with the terminal phase and structured cause retained. Statically measurable request-limit violations are instead aggregated Compilation Request Rejections. It exposes no universal retryable flag; callers derive retry policy from the cause and their context. Deterministic compiler or exporter rejection from accepted exact inputs is a Compilation Result, while dynamic limit, cancellation, process, or infrastructure failure during the same phase remains operational. An operation outcome retains the Compilation Request Inventory and dependency evidence accumulated before it terminated. Failures after terminal commitment are Compilation Delivery Outcomes or other adapter outcomes that refer to, but never replace, the immutable Compilation Report.

**Canonical Compilation Diagnostic**:
A structured compiler or exporter diagnostic identified by phase, severity, kind, logical spans, message, hints, and stable semantic ordering. Compiler diagnostics precede exporter diagnostics; deterministic engine order is preserved within each phase, and library-generated diagnostics have specified semantic positions. The phase, severity, and span envelope is stable across engine upgrades; kinds are library-defined where a real stable category exists and otherwise retain namespaced engine-specific detail scoped by exact engine identity. Warnings and errors share this one canonical collection. Terminal rendering, color, absolute host paths, operational explanations, and other adapter presentation are not part of it.

**Diagnostic Source Bundle**:
Optional owned, deduplicated exact source bytes and logical identities for sources referenced by Canonical Compilation Diagnostics, retained under an explicit operational reporting policy so adapters can render rich diagnostics after compilation returns. Generic library execution does not request it by default. It is sensitive, excluded from compilation identities, and not required for machine-readable diagnostics. Explicit limits apply, and its reporting channel distinguishes not requested, complete, limited, and unavailable collection. A cache hit remains valid when requested source context is absent and reports the channel unavailable rather than forcing re-execution.

**Compilation Timing Telemetry**:
Optional non-semantic measurements from the explicit monotonic time domain used by one lifecycle operation. Generic library execution does not request them by default, although a Compilation Attempt Deadline uses the same time domain whenever present. Preparation telemetry accompanies its Prepared Compilation or Compilation Request Rejection; a Compilation Report may report stable admission and queueing, acquisition, verification and spooling, kernel compilation, export, and finalization phases, whose parallel durations need not sum to elapsed time. Compilation Delivery reports timing separately. Finer engine timing is explicitly opt-in, best-effort, engine-specific, and potentially process-global. Each reporting channel distinguishes not requested, complete, limited, and unavailable; instrumentation failure or a cache entry without historical timing does not change the semantic result or force re-execution.

**Logical Dependency Identity**:
The canonical semantic address of a dependency, independent of where its bytes were acquired. A successful dependency observation pairs this logical identity with exact content identity; backing source identity remains Dependency Resolution Evidence rather than semantic content identity.

**Dependency Resolution Evidence**:
The canonical set of revalidatable facts that determined how a logical dependency request resolved, including content selection, authority choice, every higher-priority miss that enabled fallback, final logical missing outcomes, and relevant absence or membership. A fact is included when changing it while holding the compilation request fixed could change resolution, diagnostics, success, or artifacts. Access order, repeat counts, retries, and other causally irrelevant attempts are not semantic. A failed compilation retains the evidence accumulated through its terminal failure.

**Dependency Evidence Key**:
A sanitized stable identity and immutable version, content, absence, or membership fact supplied by an authority for Dependency Resolution Evidence. It can be revalidated or subscribed to without making credentials, absolute host paths, or transport details part of semantic identity. A backing-source notification dirties the evidence; equal revalidation preserves semantic identities and caches. Full keys live in operation results and active sessions; Packs retain portable Discovery Traces and sanitized provenance rather than source-specific revalidation handles.

**Backing Dependency Locator**:
An optional adapter projection that links a Dependency Evidence Key to a physical filesystem path, URL, object key, or equivalent location for watch and build-tool integration. It may be sensitive, is not portable, and does not enter Pack state, Compilation Identity, or Compilation Result Identity.

**Dependency Acquisition Outcome**:
The terminal typed outcome from one authority source whose result affected dependency resolution: success, unavailable or missing, transient failure, permanent failure, invalid content, or integrity mismatch. Internal retries, redirects, timing, and cache mechanics are operation telemetry rather than canonical outcomes.

**Compilation Access Trace**:
The canonical set of logical project, package, and font requests Typst actually observed during one compilation, with request kind, outcome, effective content provenance, and links to the Dependency Resolution Evidence that determined each outcome. Successful project observations identify the project path, exact selected bytes, and baseline or override use; package observations identify the Package Requirement, package-relative path, exact file bytes, and embedded or external fulfillment; font observations identify the Font Container, face index, and embedded or external fulfillment. Pack creation origin and sanitized acquisition provenance remain owned by the Pack rather than repeated here. It is distinct from the Compilation Request Inventory; access order and repeat counts are not semantic. A failed compilation retains the observations accumulated through its terminal failure.

**Reproducibility Claim**:
A claim that one fully specified Compilation Identity produces the same Compilation Result Identity under a named reproducibility and engine-compatibility level. Changed identity-bearing inputs form a different claim; later backing-source changes or unavailability do not revoke the historical claim, while a different result under the same claimed identities refutes it.

**Pack Override**:
A compilation-scoped byte replacement for any project file contained in a Pack, including Typst source and the fixed entrypoint. It cannot add or delete a path, change the entrypoint identity, or replace package or font content.
_Avoid_: placeholder, replacement asset

**Pack Override Set**:
The immutable, finite set of Pack Overrides owned by one exact compilation request and validated against one Pack Identity. Each canonical contained project path appears at most once, declaration order is not semantic, and compilation observes one fixed path-to-exact-bytes snapshot rather than consulting an override provider. Replacement bytes remain opaque unless requested as Typst source. Every member contributes to compilation identity even when its bytes are not read; observed use is separate provenance.
