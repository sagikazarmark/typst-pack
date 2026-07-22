# Typst Pack

This context describes portable Typst projects and the values that vary between individual document renders.

## Language

**Pack**:
A portable, reusable Typst compilation closure with one fixed entrypoint, established by successful Discovery Variants. It contains every observed or explicitly included project file, identifies each complete package and exact font dependency, and permits no undeclared dependency fallback. A value exists as a Pack only after canonical semantic and content-integrity validation; trust policy never weakens its validity rules.

**Discovery Variant**:
One member of a nonempty, explicitly ordered creation list, containing one exact representative compilation request: target, Typst inputs, time, enabled features, and any discovery-only project overrides. Its canonical identity is derived from that semantic request and its commitments; an optional public label and declaration order aid reporting without changing coverage identity. Canonical identities must be unique within one creation. A Pack's completeness guarantee covers the union of its recorded successful Discovery Variants and Explicit Conditional Inclusions, not arbitrary future variation; variants document coverage rather than restrict later compilation requests.

**Discovery World**:
The fixed logical project tree, entrypoint, Typst engine configuration, and package and font authorities against which every Discovery Variant in one Pack creation runs. Environment variables, wall-clock time, host fonts, caches, and other adapter defaults are resolved into this world or exact variant request values before semantic creation begins. Materially different Discovery Worlds produce separate Packs rather than variants of one Pack.

**Project Snapshot**:
The finite immutable canonical project tree and fixed entrypoint admitted as the project input to one Pack creation attempt. Every file is a Stable Byte Value, so editors, filesystems, Dagger, and other mutable or remote sources must be stabilized before crossing the creation seam. A Project Snapshot may contain files that no Discovery Variant observes and is neither a Pack nor persisted Pack state; discovery and Explicit Conditional Inclusions determine which of its files enter the Pack.

**Discovery Snapshot**:
The stable project, package, and font bytes, relevant directory membership, logical absences, and exact request values retained during one Pack creation. A mutable adapter must detect changes to consumed content or successful missing probes before issuance rather than silently create a Pack from mixed-time reads. Assembled-Pack replay uses this frozen snapshot without reacquisition; sensitive raw request values are discarded after the attempt.

**Discovery Trace**:
The canonical per-variant observation set of successful project and package reads, exact used font faces, and logical missing probes, including authority, request outcome, and effective baseline or override provenance. It is the persisted portable semantic projection of the same observation schema used by a Compilation Access Trace; source-specific Dependency Evidence Keys remain operation state. Access order and repeat counts are not semantic. A Pack's compilation closure is derived from the union of its Discovery Traces and Explicit Conditional Inclusions. The trace is persisted as discovery evidence; rendered compiler warnings and diagnostic text are not Pack state.

**Discovery Coverage Identity**:
The canonical identity binding one Discovery Variant identity to its exact Discovery Trace identity. A Pack's completeness claim is identified by the set of these bindings; variant labels and declaration order remain reporting concerns.

**Explicit Conditional Inclusion**:
An existing root-relative project file and its baseline bytes added to a Pack's compilation closure without being successfully read by a Discovery Variant. Directory and glob inputs are adapter conveniences expanded into this exact file set during creation. There is no implicit third project-file provenance category.

**Discovery Request Commitment**:
A canonical digest that identifies a potentially sensitive Discovery Variant input value or discovery-only override without storing the raw value. A commitment supports exact matching when the value is supplied again, but does not claim confidentiality against guessing.

**Pack Issuance**:
The point at which semantic creation returns a Pack after every Discovery Variant succeeds, the Discovery Snapshot is revalidated, whole-Pack invariants hold, and assembled-Pack replay reproduces every Discovery Trace. Archive or transport writing happens after issuance and commits atomically where its sink permits.

**Creation Report**:
The transient diagnostics from a Pack creation attempt, presented deterministically by validation, discovery, snapshot and assembly, replay, and issuance phase. Discovery and replay diagnostics follow Discovery Variant declaration order and identify the variant without making rendered messages part of the Pack.

**Pack Identity**:
The content identity of a Pack's canonical logical compilation state: its fixed entrypoint, discovery coverage identities, contained project files, ordered Pack Font Catalog, dependency requirements, and whether each dependency is embedded or externally fulfilled. Non-identifying provenance, archive encoding, and source-host filesystem metadata do not affect it. Matching an externally trusted expected Pack Identity detects replacement but does not authenticate a publisher; publisher identity and trust policy remain outside Pack and the semantic core.

**Canonical Identity**:
A typed, schema-versioned digest of exact bytes or a deterministic semantic projection. Identity equality includes its kind, identity schema, algorithm, and digest; a bare digest is never sufficient and acquisition provenance never changes it.
_Avoid_: bare hash, checksum

**Stable Byte Value**:
A finite immutable exact byte sequence whose owner can synchronously read every needed range for its lifetime. It may be backed by memory or by operation-owned stable local storage; physical backing, acquisition provenance, and transport location are operational rather than semantic. A stream or mutable source does not become a Stable Byte Value until acquisition is bounded and complete.

**Spool Facility**:
An explicit operation-scoped facility that boundedly acquires a sequential or mutable byte source into a Stable Byte Value. It owns partial-state cleanup on failure, cancellation, or deadline, transfers completed backing ownership to the resulting value, and never implicitly promotes content into a shared cache.

**Transport Locator**:
An adapter-owned path, URL, object key, Dagger object, service handle, or other opaque capability used to acquire or publish a value. A Transport Locator is resolved to a Stable Byte Value or immutable finite tree of Stable Byte Values, together with any exact expected identity, before crossing a semantic seam; it never becomes Pack or Compilation Result state.

**Transport Operation**:
A role-specific locator-resolution, acquisition, spooling, publication, or delivery operation behind an adapter interface. Its lifecycle begins by appraising the exact bound facility and may end in admission refusal before effects. Successful admission freezes capabilities, Deployment Trust Profile, budgets, network policy, interruption controls, and required commit and cleanup guarantees before bounded transfer, exact verification, and at most one commit. Every well-formed request returns one Transport Receipt; cancellation or deadline before commit wins, while a later signal cannot rewrite committed success.

**Transport Receipt**:
The structured terminal operational evidence returned by a Transport Operation. Its admission disposition contains either a refusal with the complete request, safely appraised descriptor projections, explicit stage `admission`, and one closed reason, or a successful Operation Admission Record followed by a role-specific reached-fact ledger. Only the admitted branch records frozen object count, reached byte counts, Content Identities, commit, cleanup outcome, exposure, and timing. Transport Locators, credentials, and raw adapter detail appear only in separately capability-gated sensitive projections.

**Pack Identity Verification Mode**:
The explicit assurance selected before Pack ingress. Verify compares one externally supplied expected Pack Identity with the identity derived from the fully validated Pack; Derive returns that derived identity while acknowledging that internal validity alone cannot detect wholesale substitution by another valid Pack. Exact Package Requirements and Font Requirements always use verification and have no Derive mode.

**Deployment Trust Profile**:
The immutable, operation-scoped security assumption and minimum enforcement contract selected before input-dependent interpretation for Pack creation, reading, compilation, projection, or delivery. Trusted assumes supplied content and executable facilities are non-adversarial; Partially Trusted treats content as externally controlled and potentially abusive while trusting deployment code and makes no same-process containment claim; Hostile includes deliberate implementation compromise and hard denial attempts and permits containment claims only under verified operating-system or runtime enforcement. Profiles never weaken semantic or integrity validation, are operational rather than identity-bearing, and are never Pack state.
_Avoid_: trusted Pack, hostile Pack, trust level, sandbox mode

**Operational Capability Class**:
A descriptive, versioned class name of at most 255 ASCII bytes for one kind of operation-causal facility, written as a reverse-DNS namespace, lower-kebab path, and positive major version. It classifies an Operation Capability Descriptor but is not an instance identity or evidence that a capability was requested, admitted, reached, enforced, or successfully exercised.

**Operation Capability Descriptor**:
The immutable, versioned, role-specific safe description owned by an exact operation-causal authority, cache, evidence provider, runtime domain, execution or reporting facility, spool, or transport adapter. It states only capabilities offered for appraisal; admission and reached facts remain separate and cannot be inferred from the descriptor, its Operational Capability Class, a concrete type, placement, or eventual success.

**Operation Admission Record**:
The immutable role-specific fact record created when a well-formed operation is successfully admitted, binding its request and exact appraised Operation Capability Descriptors to the separately requested and admitted operational controls. It never exists on refusal and contains no inferred reached fact; reports and receipt ledgers append only facts actually reached after admission.

**Operation Network Policy**:
The explicit operation-scoped request and admission choice between Network Permitted and Offline. Network Permitted allows but does not prove network use, while Offline requires every covered operation-causal facility to contractually perform no network I/O during the admitted lifecycle; structural enforcement and later operations remain separate claims.

**Portable Pack**:
A Pack that can move between hosts and compile when its exact declared external package and font bytes are supplied, without consulting the source host or undeclared ambient state. Every valid Pack provides this guarantee.

**Self-Contained Pack**:
A Pack that embeds every package and font dependency. Compiling one requires only the Pack, a compatible Typst engine, and explicit compilation request values; it does not require the host filesystem, environment, caches, installed fonts, or network. This is an offline capability of the Pack, not a claim that an attempt configured with network-capable operational facilities made no network calls.

**Pack Manifest**:
The versioned declarative description carried by a Pack. It describes intended contents and metadata, while agreement between those declarations and contained files is a Pack invariant.

**Pack Control Record**:
The canonical machine representation of a Pack Manifest shared by Pack Archive and Closure Export representations. It records the complete semantic contract plus explicitly non-identifying provenance, metadata, and annotations; readers derive and validate Pack Identity rather than trusting its claim.

**Pack Format Epoch**:
An exact, independently supported representation schema and validity contract for Pack Control Records, archive framing, and Closure Export layout. Support is an explicit finite set rather than a version range, and an unknown epoch is never interpreted approximately.

**Pack Archive Encoding**:
The adapter operation that encodes one validated Pack under one exact Archive Encoding Identity into a completed Stable Byte Value. Its receipt records the Pack Identity, Archive Encoding Identity, archive Content Identity, and exact length; publication is a later operation and cannot change encoding success.

**Pack Archive Publication**:
The adapter operation that transfers one completed Pack Archive Stable Byte Value to an authorized destination. It reports its admitted Publication Commit Strength, transfer, commit, and cleanup outcomes without changing the Pack or its encoding receipt.

**Format Receipt**:
The versioned, core-owned, role-specific terminal fact record for one well-formed Pack representation or representation-publication operation. It distinguishes refusal from admission, records only role-legal requested, admitted, and reached representation facts, and remains separate from any subject-bound Transport Receipt; malformed request construction produces no Format Receipt.

**Representation Admission Refusal**:
The typed terminal returned before effects or input-dependent interpretation when a well-formed non-publication representation request cannot be admitted under its selected or asserted archive recipe or requested operational controls. It retains the complete requested Format Receipt controls and one closed reason but no admitted or reached facts; publication instead preserves its Transport admission refusal without coercion.

**Pack Semantic Extension**:
A namespaced, versioned addition to a Pack's semantic contract that contributes to Pack Identity and must be fully understood and validated before a Pack is exposed. An unknown semantic extension makes the representation unsupported rather than partially valid.

**Pack Annotation**:
A namespaced, versioned opaque value carried and preserved by a Pack representation for presentation or external policy. It cannot affect Pack semantics, validity, dependencies, derived guarantees, or Pack Identity, and an annotation identity can never later acquire semantic meaning.

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
An explicit operation-scoped source capability for Complete Package Trees under exact Typst package specifications. It owns namespace support, source routing, fallback, acquisition, and offline policy, and reports acquisition provenance. Semantic Pack operations never consult ambient package locations outside their supplied Package Authority. Authority over acquisition does not establish trust in package bytes or publisher authenticity; every supplied tree remains independently validated.

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
An explicit operation-scoped source capability for a Font Catalog Snapshot during discovery and exact Font Containers during external fulfillment. It owns source composition and provenance, while Typst selects faces from the supplied catalog. Authority over acquisition does not establish trust in font bytes or publisher authenticity; every supplied container remains independently validated.

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
The canonical description of every value supplied to or deterministically resolved for one compilation, including values that Typst does not observe. It records whether an effective value was caller-supplied, core-defaulted, core-derived, or adapter-resolved and marks each entry as semantic or operational. Semantic entries contribute to Compilation Identity, while the Deployment Trust Profile, capability scopes, authorities, caches, retries, deadlines, isolation, transport, output destinations, filename templates, terminal rendering, and other operational controls do not unless they resolve an explicit semantic value. A Prepared Compilation owns the semantic portion; each execution report adds the operational controls used for that attempt. Potentially sensitive values are represented publicly by Compilation Request Commitments rather than raw bytes. It is distinct from resolution evidence and observed access.

**Compilation Request Commitment**:
A canonical domain-separated digest that binds a potentially sensitive compilation input value or Pack Override without exposing its raw bytes. The raw value lives only in the active request unless a caller explicitly requests sensitive adapter output; the commitment and size can appear in public inventories and identities.

**Engine Identity**:
The exact implementation identity attested by the Typst compiler used for semantic compilation. It includes every build, backend, feature, or platform compatibility distinction for which the producer does not guarantee exact behavior; callers cannot replace it with a compatibility label.

**Exporter Identity**:
The exact format-specific implementation identity attested by the exporter used to produce Compilation Output Artifacts. Like Engine Identity, it is platform-qualified wherever exact behavior is not guaranteed across implementation classes and is never caller-asserted.

**Engine-Neutral Compilation Intent**:
The canonical semantic request shared by cross-engine comparisons: Pack Identity, Pack Override Set, Typst inputs, features, Compilation Document Time, Compilation Output Specification with every effective default, and every other semantic request value, but no Engine Identity or Exporter Identity. Each compared implementation must represent these exact values without dropping, translating, or re-defaulting them. An executable Prepared Compilation adds its attested implementation identities.

**Prepared Compilation**:
An immutable, Pack-bound compilation value produced before authority access or Typst execution by requiring a Compilation Output Specification, applying core-owned deterministic defaults, deriving the Typst target and required format features, canonicalizing and validating the complete semantic request, performing strict Pack Override preflight, and attesting the core's actual engine and exporter identity. The other core defaults are an empty Typst input map and caller-selected feature set and absent Compilation Document Time. HTML derives its required feature; the accessibility feature may be selected; the unsupported bundle feature is a Compilation Request Rejection. It owns the fully explicit semantic request, the semantic portion of the Compilation Request Inventory, and the Compilation Identity. It may be executed repeatedly or concurrently; attempts share only facilities explicitly supplied through their independent Compilation Execution Controls.

**Compilation Preparation Policy**:
The immutable explicit rules for side-effect-free semantic preparation of one compilation request, including unknown-engine-feature handling and whether a Canonical Diagnostic Policy is required. One-shot compilation and Compilation Session acceptance apply the same policy before operational facilities are appraised; it is distinct from Compilation Execution Controls and Compilation Resource Limits.

**Compilation Preparation Limits**:
The immutable finite ceilings enforced only while a compilation request is prepared: Pack Override count, largest and aggregate replacement bytes, and the maximum entry count and canonical bytes that its Canonical Diagnostic Policy may declare. Exceeding them contributes to Compilation Request Rejection before a Prepared Compilation, Compilation Identity, operational admission, or report exists; they do not govern attempt execution or diagnostic projection.

**Synchronous Compilation Driver**:
The public execution mode that runs a full Compilation Attempt on its caller's thread. It resolves exact requirements through explicit synchronous authorities and caches, constructs a Compilation Dependency Snapshot, and invokes the featureless synchronous Compilation Kernel. It produces the same Compilation Report model as the Asynchronous Compilation Driver and introduces no hidden async runtime, filesystem, network, or process requirement.

**Asynchronous Compilation Driver**:
The public execution mode that runs the same full Compilation Attempt lifecycle through asynchronous authorities and caches, constructs a Compilation Dependency Snapshot, and dispatches the same Compilation Kernel through an explicit Compilation Execution Facility. Its orchestration and transport behavior do not create a second semantic compilation path.

**Compilation Kernel**:
The single synchronous semantic path shared by the Synchronous Compilation Driver and Asynchronous Compilation Driver from accepted exact dependencies through Typst compilation and complete export. It has no asynchronous interface or ambient I/O and preserves one Compilation Report contract across drivers.

**Environment-Independent Compilation**:
The semantic guarantee that, after an Engine-Neutral Compilation Intent, Engine Identity, Exporter Identity, and exact dependency bytes and outcomes are fixed, ambient filesystem state, environment variables, wall-clock time, locale, host fonts, caches, backing locations, and acquisition provenance cannot change the Compilation Result. Environment-dependent availability may still produce a Compilation Operation Outcome before a result exists, and verified network acquisition does not violate this guarantee.

**Compilation Request Rejection**:
The deterministic, side-effect-free rejection of a semantic request before a Prepared Compilation or Compilation Identity exists. It retains the supplied semantic request inventory and every independently detectable typed issue in stable order, but has no dependency evidence or access trace.

**Compilation Identity**:
The pre-execution identity of one fully specified semantic compilation request. It binds one Engine-Neutral Compilation Intent to the exact Engine Identity and Exporter Identity, including every identity-bearing value whether or not later execution observes it. Output destinations, filename templates, color, timings, sink behavior, backing source, and acquisition provenance are not part of it. Adapters resolve every ambient output-affecting default into an explicit semantic value before computing the identity; hidden ambient fallback is forbidden.

**Compilation Result Identity**:
The immutable post-execution identity of one completed compiler and exporter result after exact semantic inputs are accepted and exact dependencies are available. It binds the Compilation Identity to status, Compilation Document Summary, Canonical Compilation Diagnostics, the semantic logical/content/outcome projection of dependency observations, and every ordered Compilation Artifact Identity. Backing source keys, cache hits, acquisition provenance, timing telemetry, Diagnostic Source Bundles, and Compilation Operation Outcomes do not contribute. Later changes may supersede its currentness but do not invalidate the historical value.

**Compilation Result**:
The immutable semantic terminal value produced after exact semantic inputs are accepted and exact dependencies are available. A successful Document Format result owns exactly one Compilation Output Artifact. A successful Page Format result owns one artifact per selected source page in Source Page Number order and may own none when a valid selection matches no pages. A rejected compiler or exporter result owns no artifacts. Both statuses retain all Canonical Compilation Diagnostics, a Compilation Document Summary, the canonical Compilation Access Trace and semantic dependency projection produced by that engine attempt, and a Compilation Result Identity. Warnings never change status or remove artifacts. A semantic cache returns this same value without replacing its original trace with current cache telemetry.

**Compilation Document Summary**:
The minimal typed description of the document reached by semantic compilation. It always identifies the derived target and records the total Source Page Number count when a paged document was produced, including when later export rejects. It does not expose or retain the upstream Typst document.

**Compilation Report**:
The owned immutable account of one attempt to execute a Prepared Compilation through complete export, before artifact publication. It contains the Compilation Request Inventory complete through that seam, current-attempt Dependency Resolution Evidence and cache provenance, the terminal phase and explicit reached scope of each dependency view, and exactly one terminal Compilation Result or Compilation Operation Outcome. When no semantic result exists, it also retains the partial Compilation Access Trace reached by that attempt; otherwise the result owns the canonical trace. Operational reporting channels may add acquisition, compilation, and export timing telemetry or a sensitive Diagnostic Source Bundle; each channel reports whether it was not requested, complete, limited, or unavailable without changing the terminal semantic result. Adapters wrap this report with their additional inventory and delivery outcomes rather than mutating it or replacing its semantic result.

**Compilation Execution Controls**:
The immutable operational configuration for one Compilation Attempt: its Deployment Trust Profile and granted capability scopes, explicit synchronous or asynchronous package and font authorities, cache policy, Compilation Resource Limits, Compilation Attempt Deadline and cancellation, monotonic time domain, concurrency or isolation facilities, spooling policy where applicable, and reporting policy. An absent cache, deadline, cancellation source, or telemetry request is explicit rather than ambient. Effects and sharing occur only through these named controls; they do not contribute to Compilation Identity unless they resolve an explicit semantic value.

**Operation Resource Limits**:
The immutable finite count, byte, expansion, raster-work, memory, concurrency, queue, and reporting budgets admitted for one Pack creation, representation, compilation, projection, transport, or delivery operation. Each limit is enforced at the first seam that can observe it. A Pack Format ceiling remains universal, while exhausting a stricter Operation Resource Limit produces a typed request rejection or operational outcome and leaves validity beyond that budget unknown. Resource limits and their outcomes are operational rather than semantic identity.

**Adapter Resource Defaults**:
The documented target-specific Operation Resource Limits, concurrency caps, queue capacity, latency target, deadlines, and interruption controls an outer adapter resolves before calling the core. The core publishes no numeric default. Admitted concurrency is the smaller of a target cap and available capacity and is recorded explicitly; a caller may tighten defaults but can never raise a Pack Format ceiling. Adapter Resource Defaults do not contribute to Pack Identity, Compilation Identity, or Compilation Result Identity.

**Operation Latency Target**:
A target-specific non-terminal objective for measured queueing and operation phases. Missing it produces operational telemetry or a warning, never a semantic failure or timeout by itself. Queue timeouts and operation deadlines remain separate controls whose expiry produces typed operational outcomes at their actual enforcement strength.

**Compilation Resource Limits**:
The operational limits applied at the seam that can observe and enforce them. Drivers enforce dependency counts and declared sizes, downloaded and expanded bytes, spooling and cache budgets, and acquisition concurrency; the Compilation Kernel enforces observable logical and artifact limits; Compilation Delivery enforces transport and sink limits. In-process retained-memory and CPU limits are best-effort because Typst is non-cooperative. Only an Isolated Compilation Worker with enforced quotas may claim hard whole-attempt memory, CPU, or forced-termination bounds. Concrete budgets are adapter policy rather than semantic identity.

**Compilation Attempt**:
One execution of a Prepared Compilation under one Compilation Execution Controls snapshot, from admission through Compilation Terminal Commitment, producing one Compilation Report. A Compilation Driver never absorbs a terminal retry into the same attempt: a caller, session, or adapter retry produces a fresh report. Only bounded source-specific transport retries inside one authority acquisition remain internal operational telemetry.

**Offline Compilation Attempt**:
A Compilation Attempt whose authorities, caches, spools, execution facilities, and other controls contractually perform no network I/O from admission through Compilation Terminal Commitment. It may externally fulfill dependencies from explicit memory, local storage, or existing local caches and therefore does not require a Self-Contained Pack. Compilation Delivery is outside this scope and states its network policy separately. Hard enforcement against hostile adapters is a stronger confinement guarantee.

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
One adapter operation after Compilation Terminal Commitment that transports or durably persists an immutable Compilation Report together with its complete ordered artifact inventory. It may stream exact bytes with backpressure and commit the collection atomically where its sink permits, but it never changes the report, exposes a semantically incomplete collection as complete, or triggers recompilation.

**Compilation Delivery Outcome**:
The typed operational result of one Compilation Delivery, including the selected report disclosure, admitted Publication Commit Strength, per-object transfer, deadline, cancellation, sink, commit, and partial-state cleanup facts. It refers to the immutable Compilation Report it attempted to deliver and never replaces that report's Compilation Result or Compilation Operation Outcome.

**Compilation Report Disclosure**:
The explicit projection of an immutable Compilation Report selected for one Compilation Delivery. Identity disclosure, the safe default, binds terminal status and artifact roles and identities to the report; canonical disclosure additionally carries canonical diagnostics and operational evidence; sensitive disclosure separately admits retained source bytes, Backing Dependency Locators, and raw adapter detail under a disclosure capability. No disclosure mutates the report or its identities.

**Publication Commit Strength**:
The minimum visibility and rollback guarantee requested before a publication or delivery sink is admitted, and the actual guarantee reported afterward. Complete-collection atomic makes the whole declared collection visible at one commit or not at all; each-object atomic makes each declared object all-or-none but may expose a partial collection; streaming may expose bytes before completion and cannot promise rollback. A sink weaker than the requested strength is refused rather than silently downgraded.

**Transport Cleanup Requirement**:
The minimum partial-state cleanup guarantee requested before a Transport Operation is admitted: Complete Before Return or Residual Locator Permitted. It is a request and admission fact, never an inferred outcome.

**Transport Cleanup Outcome**:
The reached cleanup result of an admitted Transport Operation: Not Required, Complete, Residual Reported, or Cleanup Failed. Residual Reported requires an admitted Residual Locator Permitted requirement and a capability-gated locator; Complete and Not Required forbid one. Cleanup remains bounded before return, may be the first failure after commit, and never overwrites an earlier primary failure.

**Compilation Operation Outcome**:
The typed non-semantic outcome of running a Prepared Compilation before Compilation Terminal Commitment. Its closed classes cover dependency resolution and acquisition, dynamic resource limits, deadline, cancellation, supersession, execution or isolation failure, and internal integrity failure, with the terminal phase and structured cause retained. Statically measurable request-limit violations are instead aggregated Compilation Request Rejections. It exposes no universal retryable flag; callers derive retry policy from the cause and their context. Deterministic compiler or exporter rejection from accepted exact inputs is a Compilation Result, while dynamic limit, cancellation, process, or infrastructure failure during the same phase remains operational. An operation outcome retains the Compilation Request Inventory and dependency evidence accumulated before it terminated. Failures after terminal commitment are Compilation Delivery Outcomes or other adapter outcomes that refer to, but never replace, the immutable Compilation Report.

**Canonical Compilation Diagnostic**:
A structured compiler or exporter diagnostic identified by phase, severity, kind, logical spans, message, hints, and stable semantic ordering. Compiler diagnostics precede exporter diagnostics; deterministic engine order is preserved within each phase, and library-generated diagnostics have specified semantic positions. The phase, severity, and span envelope is stable across engine upgrades; kinds are library-defined where a real stable category exists and otherwise retain namespaced engine-specific detail scoped by exact engine identity. Warnings and errors share this one canonical collection. Messages, hints, and source-derived values remain untrusted and potentially sensitive presentation data; canonical structure does not make them terminal-safe, markup-safe, or publisher-authenticated. Terminal rendering, color, absolute host paths, operational explanations, and other adapter presentation are not part of it.

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
The terminal typed outcome from one authority source whose result affected dependency resolution: success, unavailable or missing, transient failure, permanent failure, invalid content, or integrity mismatch. Only unavailable or missing permits fallback; authorization denial is a permanent failure and never masquerades as absence. Internal retries, redirects, timing, and cache mechanics are operation telemetry rather than canonical outcomes.

**Compilation Access Trace**:
The canonical set of logical project, package, and font requests Typst actually observed during one compilation, with request kind, outcome, effective content provenance, and links to the Dependency Resolution Evidence that determined each outcome. Successful project observations identify the project path, exact selected bytes, and baseline or override use; package observations identify the Package Requirement, package-relative path, exact file bytes, and embedded or external fulfillment; font observations identify the Font Container, face index, and embedded or external fulfillment. Pack creation origin and sanitized acquisition provenance remain owned by the Pack rather than repeated here. It is distinct from the Compilation Request Inventory; access order and repeat counts are not semantic. A failed compilation retains the observations accumulated through its terminal failure.

**Reproducibility Claim**:
A normative, testable relation from one baseline Compilation Result to the result expected under a named reproducibility or engine-compatibility guarantee. One baseline is enough to state the claim; repeated matching execution may support it but is not required to create it. Changed identity-bearing inputs form a different claim, and later backing-source changes or unavailability do not revoke the historical claim, while a mismatching semantic result refutes it.

**Exact Reproducibility Claim**:
A Reproducibility Claim that every Compilation Result produced for one Compilation Identity has the baseline Compilation Result Identity. Engine and exporter platform qualification is already carried by that Compilation Identity. Compilation Operation Outcomes do not compete with or refute the claim unless a separate operational guarantee promised their absence.

**Cross-Engine Compatibility Claim**:
A Reproducibility Claim comparing distinct Compilation Identities derived from one Engine-Neutral Compilation Intent under named source and target Engine Identities, Exporter Identities, scope, and Cross-Engine Compatibility Level. It never makes the engine-specific Compilation Identities or Compilation Result Identities equal and never extends beyond its explicit compilation intent or finite set of Discovery Variants.

**Cross-Engine Compatibility Level**:
One of four cumulative, testable strengths. Request Compatible means both implementations can prepare the same fully explicit Engine-Neutral Compilation Intent. Closure Compatible additionally means the target reaches a Compilation Result using only the Pack contract and exact declared fulfillments, without undeclared fallback. Structurally Compatible additionally requires equal result status, Compilation Document Summary, portable diagnostic envelope, semantic dependency projection, and ordered artifact roles, but not equal artifact bytes, diagnostic wording or hints, or engine-specific diagnostic detail. Exactly Reproducible additionally requires equality of the complete engine-neutral result projection, including full Canonical Compilation Diagnostics and exact artifact bytes.

**Pack Override**:
A compilation-scoped byte replacement for any project file contained in a Pack, including Typst source and the fixed entrypoint. It cannot add or delete a path, change the entrypoint identity, or replace package or font content.
_Avoid_: placeholder, replacement asset

**Pack Override Set**:
The immutable, finite set of Pack Overrides owned by one exact compilation request and validated against one Pack Identity. Each canonical contained project path appears at most once, declaration order is not semantic, and compilation observes one fixed path-to-exact-bytes snapshot rather than consulting an override provider. Replacement bytes remain opaque unless requested as Typst source. Every member contributes to compilation identity even when its bytes are not read; observed use is separate provenance. The set is semantic request data rather than an authorization grant; an adapter authorizes who may supply it separately.

**Compilation Session**:
A caller-owned, Pack-bound coordinator for repeatedly evaluating changing compilation request state. It converts each accepted state into exact immutable attempt inputs and owns revision ordering, dependency currentness, latest-only scheduling, and publication state. Authorities, caches, watchers, clocks, runtimes, execution facilities, and delivery remain explicit caller facilities. A different Pack Identity starts a different Compilation Session.

**Compilation Session Revision**:
The session-local monotonic identity of one desired evaluation state: an immutable semantic request snapshot, effective session policy, and the dependency dirtiness that must be reconciled before it can be current. An effective request or policy change, dependency invalidation, or explicit refresh creates a new revision; an operational retry remains within its revision. Revisions are operational, do not contribute to Compilation Identity, and may resolve to the same Compilation Identity or Compilation Result as another revision.

**Session Evaluation**:
The session-local, revision-bound ordered identity of one pass that produces and reconciles a terminal candidate. Accepting a revision creates its first evaluation, while an explicit operational retry creates another evaluation in the same revision without changing its prepared semantic request or policy; attempts, fences, publications, and Last Successful Compilation retain the evaluation that produced them.

**Session Ingestion Failure**:
The typed, tokenless terminal candidate produced when an adapter cannot stabilize the request sources needed to accept a compilation request into a Compilation Session. It retains the failed request-source scopes and effective policy, invokes no preparation, creates no attempt or semantic terminal, and may publish only as stale or unverified without replacing Last Successful Compilation.

**Superseded Session Attempt**:
A token-bound Compilation Attempt whose session-owned supersession permit has been synchronously revoked because a newer Compilation Session Revision became latest. It cannot publish; a matching late completion is still consumed to clear the draining slot and activate the latest eligible pending work, while any already committed Compilation Report remains immutable.

**Session Retirement**:
The irreversible shutdown protocol that moves a Compilation Session from Running through Retiring to Retired. It rejects new input and retries, revokes unpublished work, retires subscriptions, and interrupts attempts, reaching Retired only after live attempt and subscription-arming resources are returned, reaped, or proven abandoned; recovery creates a new Session instance.

**Dependency Change Notification**:
A non-semantic hint that one Dependency Evidence Key or a declared provider scope may no longer describe current backing state. It never supplies trusted replacement evidence: the affected facts must be revalidated. Duplicate, reordered, or coarse notifications may be coalesced, while a detected delivery gap dirties the complete affected scope.

**Session Watch Coverage**:
The explicit operational claim describing whether every mutable declared request source and every causal Dependency Evidence Key for a Compilation Session is immutable or covered by race-free notification and revalidation. Complete push coverage detects notification loss and closes read-to-subscribe gaps; complete polling coverage guarantees convergence after a coherent poll once relevant state quiesces. Incomplete coverage identifies the uncovered scopes and never presents a result as known current for them.

**Session Currentness**:
The explicit operational relation between a published terminal evaluation and the mutable state watched by a Compilation Session. Healthy complete push coverage may claim reconciliation through its latest provider cursors; complete polling coverage claims only the state observed by its last coherent poll; incomplete coverage remains unverified. An accepted change, dirty notification, notification gap, or coverage downgrade makes the prior publication stale until the required revalidation succeeds. Currentness never changes the validity of an immutable historical Compilation Result.

**Session Publication**:
The atomic transition that exposes a terminal evaluation and its Session Currentness for a Compilation Session Revision. Only the latest desired revision may publish; its coverage and evidence determine whether it is current, current only as of an observation, or unverified. A report committed before later supersession remains an immutable historical value but cannot publish afterward. Session publication is distinct from progress reporting, Compilation Terminal Commitment, and Compilation Delivery.

**Last Successful Compilation**:
The most recent succeeded Compilation Result that won Session Publication. It remains one immutable whole result and artifact set, retains its originating session revision and result identity, and is explicitly stale whenever it is not current, including after a notification gap or watch-coverage downgrade. Rejection, operational failure, supersession, or delivery failure does not replace it; last successful delivery is separate adapter state.
