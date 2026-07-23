# Typst Pack

This context describes portable Typst projects and the values that vary between individual document renders.

## Language

**Pack**:
A portable, reusable Typst compilation closure with one fixed entrypoint. Its project closure contains every eligible regular file in its Project Snapshot except paths excluded by the Project Ignore Policy, independently of which files creation compilation reads. Every project path needed by its Creation Request has contained baseline bytes; later compilation may replace those bytes only through Pack Overrides. It identifies each complete package and exact font dependency selected by successful creation compilation and permits no undeclared dependency fallback. A value exists as a Pack only after canonical semantic and content-integrity validation; trust policy never weakens its validity rules.

**Creation Request**:
The one exact representative source-evaluation request used during Pack creation. It contains the effective Typst target, inputs, Compilation Document Time, and engine features, but no Pack Overrides or exporter controls. It selects package and font dependencies and must compile successfully, but it does not select project files or restrict the Pack's later output formats. It is transient operation state and does not contribute to Pack Identity.

**Creation World**:
The fixed Project Snapshot, entrypoint, Typst engine configuration, and package and font authorities against which the Creation Request runs. Environment variables, wall-clock time, host fonts, caches, and other adapter defaults are resolved into this world or exact request values before semantic creation begins.

**Project Snapshot**:
The finite immutable canonical project tree and fixed entrypoint admitted as the project input to one Pack creation attempt. It contains only regular files beneath the physical project root; symlinks and other filesystem entry kinds are not project files. Every eligible regular file is a Stable Byte Value, so editors, filesystems, Dagger, and other mutable or remote sources must be stabilized before crossing the creation seam. The Project Ignore Policy determines which root-relative paths are excluded from the project closure; compiler observations do not select project files. A mutable filesystem adapter must prove stable eligible membership and bytes through the Creation Evidence Fence, while changes confined to conclusively ignored subtrees are irrelevant. Unsupported unignored entries, unreadable eligible files, unrepresentable paths, traversal failures, and invalid ignore policy representations prevent snapshot construction. Source locators, Dependency Evidence Keys, and revalidation capabilities remain operation state outside this semantic value. A Project Snapshot is neither a Pack nor persisted Pack state.

**Project Ignore Policy**:
The root-scoped exclusion policy applied before Pack creation. Every `.typk` file is excluded by a non-overridable built-in rule so prior Pack outputs cannot recursively enter Pack Identity. Additional ordered rules use a Gitignore-style matching model: comments and blank rules, negation with last-match precedence, root anchoring, directory-only rules, basename matching, and recursive wildcards. A negated descendant may require traversal through an otherwise excluded directory. The root policy representation is always included regardless of its rules. Package trees and font sources are outside its scope. A filesystem CLI reads additional rules from the project root's `.typkignore`; nested files of that name are ordinary project files.

**Creation Snapshot**:
The stable project, package, and font bytes, relevant directory membership, logical absences, and exact request values retained during one Pack creation. Operation-scoped source and authority evidence links these values to a Creation Evidence Fence without becoming semantic state. Sensitive raw request values and operational evidence are discarded after the attempt.

**Creation Evidence Fence**:
The operation-scoped proof established after creation compilation selects package and font state and before assembly that every mutable source fact causally used to stabilize creation inputs or select project, package, and font state still agrees with the Creation Snapshot. A source satisfies the fence through immutable or versioned evidence, or through race-closing revalidation of all relevant content, absence, membership, order, metadata, and source-choice Dependency Evidence Keys. A changed, incomplete, or concurrently dirtied fact prevents Pack Issuance; keys, revalidation capabilities, and backing locations are never Pack state.

**Pack Issuance**:
The point at which semantic creation returns a Pack after its Creation Request succeeds, the Creation Snapshot is revalidated, and whole-Pack invariants hold. Archive or transport writing happens after issuance and commits atomically where its sink permits.

**Creation Report**:
The transient diagnostics and dependency observations from a Pack creation attempt, presented deterministically by validation, snapshot, compilation, assembly, and issuance phase. Rendered compiler messages and access observations are not Pack state.

**Pack Identity**:
The content identity of a Pack's canonical logical compilation state: its fixed entrypoint, contained project files, ordered Pack Font Catalog, dependency requirements, and whether each dependency is embedded or externally fulfilled. The transient Creation Request, non-identifying provenance, archive encoding, and source-host filesystem metadata do not affect it. Matching an externally trusted expected Pack Identity detects replacement but does not authenticate a publisher; publisher identity and trust policy remain outside Pack and the semantic core.

**Canonical Identity**:
A typed, schema-versioned digest of exact bytes or a deterministic semantic projection. Identity equality includes its kind, identity schema, algorithm, and digest; a bare digest is never sufficient and acquisition provenance never changes it.
_Avoid_: bare hash, checksum

**Stable Byte Value**:
A finite immutable exact byte sequence whose owner can synchronously read every needed range for its lifetime. It may be backed by memory or by operation-owned stable local storage; physical backing, acquisition provenance, and transport location are operational rather than semantic. A stream or mutable source does not become a Stable Byte Value until acquisition is bounded and complete.

**Cache Isolation Domain**:
The operational authorization and confidentiality partition within which a cache adapter permits lookup, admission, and sharing. It may distinguish a tenant, principal, workspace, authority composition, or deployment policy, but never contributes to Pack Identity, Compilation Identity, Compilation Result Identity, or content identity. Matching an identity does not authorize access across domains.

**Content Cache**:
An explicit caller- or adapter-owned operational facility that retains complete immutable values by typed Canonical Identity and exact length. A hit is independently authorized and verified before semantic use; it does not establish source authority, provenance, freshness, or publisher authenticity. Source movement never invalidates retained content, eviction changes only availability for reuse, and successful spooling never implies cache admission.

**Spool Facility**:
An explicit operation-scoped facility that boundedly acquires a sequential or mutable byte source into a Stable Byte Value. It owns partial-state cleanup on failure, cancellation, or deadline, transfers completed backing ownership to the resulting value, and never implicitly promotes content into a shared cache.

**Representation Operation**:
A role-specific operation that reads or imports a Pack representation into a validated Pack, or encodes or projects a validated Pack into a complete immutable representation. Pack Archive reading and encoding and Closure Export import and projection are Representation Operations; acquisition, spooling, publication, and delivery are separate Transport Operations. A Representation Operation starts only after typed request construction and any independently performed input stabilization.

**Format Receipt**:
The core-owned role-specific evidence for a Representation Operation, or the format-side projection paired with representation publication. It retains request-derived identities and expectations, its admission disposition, and only the format validation, construction, encoding, or publication facts reached for that role. A well-formed request refused before representation effects or interpretation still produces a Format Receipt with its request facts and closed refusal reason, but no admitted or output facts.

**Representation Admission Refusal**:
The core-owned refusal of a well-formed Representation Operation request before representation effects or input interpretation. It retains the complete request and one closed reason, including an unsupported Archive Encoding Identity, but owns no Operation Admission Record and establishes no output, validity, or assertion-comparison fact. Failure to construct the typed request occurs earlier and produces no role receipt.

**Transport Locator**:
An adapter-owned path, URL, object key, Dagger object, service handle, or other opaque capability used to acquire or publish a value. A Transport Locator is resolved to a Stable Byte Value or immutable finite tree of Stable Byte Values, together with any exact expected identity, before crossing a semantic seam; it never becomes Pack or Compilation Result state.

**Transport Operation**:
A role-specific locator-resolution, acquisition, spooling, publication, or delivery operation behind an adapter interface. Its lifecycle begins by appraising the exact bound facility and may end in admission refusal before plan freezing or effects. Successful admission freezes capabilities, Deployment Trust Profile, budgets, network policy, interruption controls, and required commit and cleanup guarantees before bounded transfer, exact verification, and at most one commit. Every well-formed request returns one Transport Receipt; cancellation or deadline before commit wins, while a later signal cannot rewrite committed success.

**Transport Receipt**:
The structured terminal operational evidence returned by a Transport Operation. Its Transport Admission Disposition contains either a refusal with the complete request, safely appraised descriptor projections, and one closed reason, or a successful Operation Admission Record followed by a role-specific Transport Stage Ledger. Only the admitted branch records reached byte counts, Content Identities, commit, cleanup, exposure, timing, and subject facts. Transport Locators, credentials, and raw adapter detail appear only in separately capability-gated sensitive projections.

**Transport Admission Disposition**:
The closed admission branch in a Transport Receipt. Refused contains the complete requested controls, safe projections of the exact capability descriptors appraised before refusal, and one closed refusal reason; it contains no admitted or actual fields. Admitted contains one Operation Admission Record and permits reached transport facts. A transport role that was never requested has no receipt rather than a fabricated disposition.

**Transport Stage Ledger**:
The sealed role-specific account of stages actually reached after transport admission. It uses a closed common vocabulary for plan freezing, reference resolution, acquisition, spooling, transfer, verification, commit, cleanup, and completion, while each transport role fixes its legal subset and order. It records optional stages and cleanup after a primary failure without asking consumers to infer them from one highest stage.

**Pack Identity Verification Mode**:
The explicit assurance selected before Pack ingress. Verify compares one externally supplied expected Pack Identity with the identity derived from the fully validated Pack; Derive returns that derived identity while acknowledging that internal validity alone cannot detect wholesale substitution by another valid Pack. Exact Package Requirements and Font Requirements always use verification and have no Derive mode.

**Deployment Trust Profile**:
The immutable, operation-scoped security assumption and minimum enforcement contract selected before input-dependent interpretation for Pack creation, reading, compilation, projection, or delivery. Trusted assumes supplied content and executable facilities are non-adversarial; Partially Trusted treats content as externally controlled and potentially abusive while trusting deployment code and makes no same-process containment claim; Hostile includes deliberate implementation compromise and hard denial attempts and permits containment claims only under verified operating-system or runtime enforcement. Profiles never weaken semantic or integrity validation, are operational rather than identity-bearing, and are never Pack state.
_Avoid_: trusted Pack, hostile Pack, trust level, sandbox mode

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

**Archive Encoding Identity**:
The Canonical Identity of one registered deterministic Pack Archive writer recipe. A typed identity may be selected for encoding or supplied as an Archive Encoding Assertion before the implementation supports or evaluates it; retaining that request fact does not establish recipe use or byte equality. Archive framing and compression methods never imply an Archive Encoding Identity.

**Archive Encoding Assertion**:
An optional Pack Archive read request to prove that the exact supplied archive bytes equal the output of one supported Archive Encoding Identity. Its comparison status is not asserted, supplied but unevaluated, byte verified, or byte mismatched. Unsupported identities cause a Representation Admission Refusal, while an earlier-precedence terminal may also leave a supported assertion supplied but unevaluated; neither case permits an inferred identity.

**Pack Archive Encoding**:
The Representation Operation that encodes one validated Pack under one exact Archive Encoding Identity into a completed Stable Byte Value. Its Format Receipt retains the selected identity even when support appraisal refuses the request without claiming recipe use or output; success additionally records the Pack Identity, archive Content Identity, and exact length. Publication is a later operation and cannot change encoding success.

**Pack Archive Publication**:
The Transport Operation that transfers one completed Pack Archive Stable Byte Value to an authorized destination. Its paired Format and Transport Receipts are core-owned projections of one private publication admission and reached-fact record plus the immutable archive description; they report coherent requested, admitted, transfer, commit, cleanup, and exposure facts without changing the Pack or its encoding receipt.

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
Every addressable regular file beneath an acquired package root, represented by its canonical package-relative path and bytes. It includes files not read by the Creation Request and package metadata, but excludes empty directories, archive encoding, and source-host filesystem metadata.

**Package Authority**:
An explicit operation-scoped source capability for Complete Package Trees under exact Typst package specifications. It owns namespace support, source routing, fallback, acquisition, and offline policy, and reports acquisition provenance. Semantic Pack operations never consult ambient package locations outside their supplied Package Authority. Authority over acquisition does not establish trust in package bytes or publisher authenticity; every supplied tree remains independently validated.

**Package Acquisition Provenance**:
A sanitized, non-identifying summary of the authority and source class from which a Complete Package Tree was acquired. It excludes credentials, transient attempts, and absolute host paths, and does not contribute to Package Requirement or Pack Identity.

**Offline Package Authority**:
A Package Authority that performs no network acquisition. It may still serve explicitly supplied, in-memory, configured local, or previously cached Complete Package Trees.

**External Package Fulfillment**:
Supplying a non-embedded Package Requirement through the compilation's explicit Package Authority. Fulfillment is directed by the requirement's logical and content identities rather than a serialized source location, and the supplied Complete Package Tree is independently verified before use.

**Package Embedding Policy**:
A deterministic choice, made independently for each Package Requirement after creation dependency selection, to embed its Complete Package Tree or require External Package Fulfillment. The choices affect Pack Identity and determine whether the Pack is self-contained.

**Font Container**:
The exact bytes of one standalone font file or multi-face font collection. Its canonical content identity is independent of source location, and all faces in a collection travel as one container.

**Font Face Identity**:
An exact face within a Font Container, identified by the container's content identity and the face's container-local index. Names, style, coverage, and other face metadata are derived from the verified container rather than independent identity.

**Font Requirement**:
A Pack dependency consisting of one Font Container identity and the nonempty set of Font Face Identities selected by the Creation Request. Identical container bytes acquired from different locations form one requirement.

**Font Catalog Snapshot**:
The finite, explicitly ordered set of candidate face metadata and stable Font Container acquisition identities fixed for one Creation World. An adapter prepares it from intentional sources before creation compilation; selected container bytes are frozen on first use rather than every candidate being read eagerly.

**Font Authority**:
An explicit operation-scoped source capability for a Font Catalog Snapshot during creation and exact Font Containers during external fulfillment. It owns source composition and provenance, while Typst selects faces from the supplied catalog. Authority over acquisition does not establish trust in font bytes or publisher authenticity; every supplied container remains independently validated.

**Offline Font Authority**:
A Font Authority that performs no network acquisition. It may still serve explicitly supplied, in-memory, configured local, system, engine-embedded, or previously cached Font Containers.

**Font Acquisition Provenance**:
A sanitized, non-identifying summary of the authority and source class from which a Font Container was acquired. It excludes credentials, transient attempts, and absolute host paths, and does not contribute to Font Requirement or Pack Identity.

**Font Scan Policy**:
The declared rules a Font Authority uses to omit, warn about, or reject invalid and unreadable candidates while constructing a Font Catalog Snapshot. The policy is fixed within one Creation World, and its diagnostics are never silently discarded.

**Font Licensing Metadata**:
Advisory standardized licensing and embedding fields derived from a verified Font Container for policy and reporting. They do not establish legal permission to redistribute or embed the font.

**Pack Font Catalog**:
The ordered projection of a Creation World's Font Catalog Snapshot containing exactly the Font Face Identities required by a Pack. It preserves their relative creation-selection order; other faces physically present in a required Font Container remain unavailable to compilation.

**Font Embedding Policy**:
A deterministic choice, made independently for each Font Requirement after creation dependency selection, to embed its Font Container or require External Font Fulfillment. The policy may inspect Font Licensing Metadata, and its choices affect Pack Identity and self-containment.

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
The canonical description of every value supplied to or deterministically resolved for one compilation, including values that Typst does not observe. It records whether an effective value was caller-supplied, core-defaulted, core-derived, or adapter-resolved and marks each entry as semantic or operational. Semantic entries contribute to Compilation Identity, while the Deployment Trust Profile, capability scopes, authorities, caches, retries, deadlines, isolation, transport, output destinations, filename templates, terminal rendering, and other operational controls do not unless they resolve an explicit semantic value. A Prepared Compilation owns the semantic portion; each execution report adds the operational controls used for that attempt. Safe projections represent potentially sensitive Typst inputs and Pack Overrides only by role-bound Compilation Request Commitments and exact sizes, never by raw bytes, replacement Content Identities, baseline-comparison identities, or byte-equality status. Observed use belongs to the Compilation Access Trace rather than this pre-execution inventory.

**Compilation Request Commitment**:
A canonical domain-separated digest that binds a potentially sensitive compilation input value without exposing its raw bytes. A Pack Override commitment binds its schema and role, Pack Identity, canonical project path, exact byte length, and exact replacement bytes; other roles have equally explicit transcripts. The raw value lives only in the active request unless a caller explicitly requests an authorized sensitive projection. A commitment and exact size may appear in safe inventories and identities, but the commitment does not provide confidentiality against guessing.

**Engine Identity**:
The exact implementation identity attested by the Typst compiler used for semantic compilation. It includes every build, backend, feature, or platform compatibility distinction for which the producer does not guarantee exact behavior; callers cannot replace it with a compatibility label.

**Exporter Identity**:
The exact format-specific implementation identity attested by the exporter used to produce Compilation Output Artifacts. Like Engine Identity, it is platform-qualified wherever exact behavior is not guaranteed across implementation classes and is never caller-asserted.

**Engine-Neutral Compilation Intent**:
The canonical semantic request shared by cross-engine comparisons: Pack Identity, Pack Override Set, Typst inputs, features, Compilation Document Time, Compilation Output Specification with every effective default, Canonical Diagnostic Policy, and every other semantic request value, but no Engine Identity or Exporter Identity. Each compared implementation must represent these exact values without dropping, translating, or re-defaulting them. An executable Prepared Compilation adds its attested implementation identities.

**Prepared Compilation**:
An immutable, Pack-bound compilation value produced before authority access or Typst execution by requiring a Compilation Output Specification, applying core-owned deterministic defaults, deriving the Typst target and required format features, canonicalizing and validating the complete semantic request, performing strict Pack Override preflight, and attesting the core's actual engine and exporter identity. The other core defaults are an empty Typst input map and caller-selected feature set and absent Compilation Document Time. HTML derives its required feature; the accessibility feature may be selected; the unsupported bundle feature is a Compilation Request Rejection. It owns the fully explicit semantic request, including its effective Canonical Diagnostic Policy, the semantic portion of the Compilation Request Inventory, and the Compilation Identity. It may be executed repeatedly or concurrently; attempts share only facilities explicitly supplied through their independent Compilation Execution Controls.

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

**Compilation Terminal**:
The closed core terminal returned by a one-shot compilation after a validated Pack and exact semantic request exist: either one Compilation Request Rejection or one Compilation Report. It has no identity of its own and does not flatten a report's Compilation Result or Compilation Operation Outcome into competing branches. Pack ingress, adapter ingestion, and default-resolution failures occur before this terminal; Compilation Delivery and other adapter work occur after a report and retain that immutable report by reference.

**Compilation Identity**:
The pre-execution identity of one fully specified semantic compilation request. It binds one Engine-Neutral Compilation Intent to the exact Engine Identity and Exporter Identity, including every identity-bearing value whether or not later execution observes it. Output destinations, filename templates, color, timings, sink behavior, backing source, and acquisition provenance are not part of it. Adapters resolve every ambient output-affecting default into an explicit semantic value before computing the identity; hidden ambient fallback is forbidden.

**Compilation Result Identity**:
The immutable post-execution identity of one completed compiler and exporter result after exact semantic inputs are accepted and exact dependencies are available. It binds the Compilation Identity to status, Compilation Document Summary, Canonical Diagnostic Envelope, the semantic logical/content/outcome projection of dependency observations, and every ordered Compilation Artifact Identity. Backing source keys, cache hits, acquisition provenance, timing telemetry, Diagnostic Source Bundles, Diagnostic Projections, and Compilation Operation Outcomes do not contribute. Later changes may supersede its currentness but do not invalidate the historical value.

**Compilation Result**:
The immutable semantic terminal value produced after exact semantic inputs are accepted and exact dependencies are available. A successful Document Format result owns exactly one Compilation Output Artifact. A successful Page Format result owns one artifact per selected source page in Source Page Number order and may own none when a valid selection matches no pages. A rejected compiler or exporter result owns no artifacts. Both statuses retain one Canonical Diagnostic Envelope, a Compilation Document Summary, the canonical Compilation Access Trace and semantic dependency projection produced by that engine attempt, and a Compilation Result Identity. Warnings never change status or remove artifacts, and a limited diagnostic envelope never changes a separately determined status. A semantic cache returns this same value without replacing its original trace with current cache telemetry.

**Semantic Result Cache**:
An explicit caller-owned operational facility that maps one Compilation Identity to at most one complete immutable Compilation Result, including its complete artifact set and original Compilation Access Trace. It admits succeeded and rejected semantic results but never request rejections, Compilation Operation Outcomes, current-attempt evidence, timing, delivery state, or source bundles. A hit can satisfy compilation without reacquisition but cannot establish Session Currentness, and only an authorized trusted or authenticated writer domain may supply a reusable result.

**Compilation Document Summary**:
The minimal typed description of the document reached by semantic compilation. It always identifies the derived target and records the total Source Page Number count when a paged document was produced, including when later export rejects. It does not expose or retain the upstream Typst document.

**Compilation Report**:
The owned immutable account of one attempt to execute a Prepared Compilation through complete export, before artifact publication. It contains the Compilation Request Inventory complete through that seam, current-attempt Dependency Resolution Evidence and cache provenance, the terminal phase and explicit reached scope of each dependency view, and exactly one terminal Compilation Result or Compilation Operation Outcome. When no semantic result exists, it also retains the partial Compilation Access Trace reached by that attempt; otherwise the result owns the canonical trace. A semantic-cache hit preserves the cached result and original trace while the fresh report records hit verification, skipped acquisition, and unavailable or separately reconstructed current evidence. Operational reporting channels may add acquisition, compilation, and export timing telemetry or a sensitive Diagnostic Source Bundle; each channel reports whether it was not requested, complete, limited, or unavailable without changing the terminal semantic result. Adapters wrap this report with their additional inventory and post-commit outcomes rather than mutating it or replacing its semantic result.

**Compilation Execution Controls**:
The immutable operational configuration for one Compilation Attempt: its Deployment Trust Profile and granted capability scopes, explicit synchronous or asynchronous package and font authorities, cache policy, Compilation Resource Limits, Compilation Attempt Deadline and cancellation, monotonic time domain, concurrency or isolation facilities, spooling policy where applicable, and reporting policy. An absent cache, deadline, cancellation source, or telemetry request is explicit rather than ambient. Effects and sharing occur only through these named controls; they do not contribute to Compilation Identity unless they resolve an explicit semantic value.

**Operational Capability Class**:
A stable, namespaced, versioned, report-safe category for an authority, cache, evidence provider, Engine Runtime Domain, execution facility, spool, or other operational facility. It describes the kind of facility without exposing a concrete Rust type, credential, locator, or private implementation detail. The class is descriptive rather than proof of a capability, trust guarantee, or enforcement claim; those remain explicit closed fields in the facility's Operation Capability Descriptor.

**Operation Capability Descriptor**:
An immutable, operation-safe declaration captured from the exact operational facility before admission. A role-specific descriptor binds an Operational Capability Class to the facility internally and states only its explicit supported network, cache, evidence, execution, interruption, isolation, enforcement, and reporting capabilities. It is neither an executable capability nor an operation request, admission, reached outcome, or authenticity claim, and its internal facility binding is not ordinary report disclosure.

**Operation Admission Record**:
The immutable operation-bound result of successfully matching complete requested operational controls against the exact role-specific Operation Capability Descriptors before effects begin. It records the selected profile provenance and explicit requested, admitted, and not-applicable trust, resource, network, concurrency, execution, interruption, isolation, publication, cleanup, and reporting values. Reports and receipts retain its report-safe projection and append reached facts without inferring missing state from a profile, concrete type, placement, or observed success. A refusal before this record exists is a separate typed admission refusal and never fabricates an admission record or core report.

**Operation Network Policy**:
The explicit requested and admitted contract for network I/O attributable to one operation through its semantic terminal commitment. Offline admission requires every selected evidence provider, authority and private cache, semantic cache, spool, execution facility, worker-control transport, and other causally usable operation facility to forbid network I/O; an observed lack of network calls is not a substitute. Contractual no-network behavior and verified structural enforcement are reported separately, while publication and delivery have their own network policies.

**Operation Resource Limits**:
The immutable finite count, byte, expansion, raster-work, memory, concurrency, queue, and reporting budgets admitted for one Pack creation, representation, compilation, projection, transport, or delivery operation. Each limit is enforced at the first seam that can observe it. A Pack Format ceiling remains universal, while exhausting a stricter Operation Resource Limit produces a typed request rejection or operational outcome and leaves validity beyond that budget unknown. Resource limits and their outcomes are operational rather than semantic identity. They may limit Diagnostic Projections and Diagnostic Source Bundles but never select or truncate the identity-bearing Canonical Diagnostic Envelope.

**Adapter Resource Defaults**:
The versioned, documented target-specific Operation Resource Limits, concurrency caps, queue capacity, latency target, deadlines, interruption controls, and engine policy that a shipped first-party adapter resolves before calling the core. The core publishes no numeric default for these operational controls. Requested and admitted values are recorded explicitly; a caller may tighten defaults but can never raise a Pack Format ceiling or an adapter's admitted capacity. Adapter Resource Defaults do not contribute to Pack Identity, Compilation Identity, or Compilation Result Identity. An adapter may also resolve a documented Canonical Diagnostic Policy, but that separate semantic value is recorded in request inventory and does contribute to Compilation Identity.

**Reference Resource Profile**:
A non-binding worked example of Adapter Resource Defaults for an adapter or deployment typst-pack does not ship. It is neither a core default nor a support, capacity, confinement, or compatibility claim. A consumer may adopt it only by selecting, versioning, enforcing, testing, and reporting its own effective defaults.

**Operation Latency Target**:
A target-specific non-terminal objective for measured queueing and operation phases. Missing it produces operational telemetry or a warning, never a semantic failure or timeout by itself. Queue timeouts and operation deadlines remain separate controls whose expiry produces typed operational outcomes at their actual enforcement strength.

**Compilation Resource Limits**:
The operational limits applied at the seam that can observe and enforce them. Drivers enforce dependency counts and declared sizes, downloaded and expanded bytes, spooling and cache budgets, and acquisition concurrency; the Compilation Kernel enforces observable logical and artifact limits; Compilation Delivery enforces transport and sink limits. In-process retained-memory and CPU limits are best-effort because Typst is non-cooperative. Only an Isolated Compilation Worker with enforced quotas may claim hard whole-attempt memory, CPU, or forced-termination bounds. Concrete budgets are adapter policy rather than semantic identity.

**Compilation Attempt**:
One execution of a Prepared Compilation under one Compilation Execution Controls snapshot, from admission through Compilation Terminal Commitment, producing one Compilation Report. A Compilation Driver never absorbs a terminal retry into the same attempt: a caller, session, or adapter retry produces a fresh report. Only bounded source-specific transport retries inside one authority acquisition remain internal operational telemetry.

**Offline Compilation Attempt**:
A Compilation Attempt whose authorities, caches, spools, execution facilities, and other controls contractually perform no network I/O from admission through Compilation Terminal Commitment. It may externally fulfill dependencies from explicit memory, local storage, or existing local caches and therefore does not require a Self-Contained Pack. Compilation Delivery is outside this scope and states its network policy separately. Hard enforcement against hostile adapters is a stronger confinement guarantee.

**Engine Runtime Domain**:
The operational sharing and accounting scope of one exact engine and exporter runtime's process- or instance-wide facilities, including internal parallelism, memoization, interners, and fine engine timing. Its parallelism policy is fixed before managed engine work begins and cannot vary by Prepared Compilation or Compilation Attempt. Sharing a domain grants no per-attempt fairness, retained-memory, timing-isolation, or tenant-confidentiality guarantee; callers that require those properties use separate process or runtime domains. Domain policy and placement are operational and do not contribute to compilation identities.

**Compilation Execution Facility**:
An explicit caller-supplied facility used by the Asynchronous Compilation Driver to admit, queue, and run synchronous Compilation Kernel and exporter jobs after their Compilation Dependency Snapshots are complete. Its queue and worker caps apply only to ready dispatches, independently of dependency-acquisition and transport concurrency; a worker slot is held until the dispatched job returns with no surviving work. It may be intentionally shared across attempts but owns no semantic state; queue time consumes each attempt's Compilation Attempt Deadline, and fairness and completion order are not semantic. The Synchronous Compilation Driver instead runs on its caller's thread, and typst-pack owns no global scheduler or whole-attempt admission policy.

**Isolated Compilation Worker**:
An optional process behind a versioned Compilation Execution Facility protocol that receives only a Prepared Compilation, a verified read-only Compilation Dependency Snapshot or confined handles to it, resource controls, and the exact engine and exporter implementation. It owns one fixed Engine Runtime Domain and runs at most one dispatched job at a time unless its declared sharing contract permits overlap. Acquisition, credentials, mutable caches, and asynchronous I/O remain in the parent. The parent exposes no output before it verifies a complete report, never silently falls back in-process, and reports worker crash, protocol, and limit failures as isolation outcomes. Isolation guarantees killability and resource placement, not hostile-input sandboxing by itself.

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
The explicit capability-gated projection of a Compilation Terminal or immutable Compilation Report selected for rendering, response, or Compilation Delivery. Identity disclosure is the safe default and carries the terminal branch, identities, artifact roles and identities, and diagnostic envelope counts and completion state without exact diagnostic text. Exact canonical messages and hints, Diagnostic Source Bundles, raw request values and Pack Override bytes, Backing Dependency Locators, and raw adapter detail are independent explicit capabilities rather than one escalating disclosure level. Every projection identifies its originating terminal or result and its own complete, redacted, limited, or unavailable status; no disclosure mutates or substitutes for the source value.

**Publication Commit Strength**:
The visibility and rollback guarantee for a publication or delivery sink, recorded separately as the requested minimum, exact admitted promise, and reached actual commit fact. Complete-collection atomic makes the whole declared collection visible at one commit or not at all; each-object atomic makes each declared object all-or-none but may expose a partial collection; streaming may expose bytes before completion and cannot promise rollback. A sink weaker than the requested minimum is refused rather than silently downgraded, and a commit not reached has no actual strength.

**Transport Cleanup Requirement**:
The minimum bounded disposition of adapter-owned partial state requested before a Transport Operation is admitted and the exact guarantee admitted for it: complete cleanup before return or permission to report a capability-gated residual Transport Locator. It is independent of Publication Commit Strength and non-retractable external exposure.

**Transport Cleanup Outcome**:
The reached disposition of adapter-owned partial state after a Transport Operation: not required, complete, residual reported, or cleanup failed. Exact non-retractable exposed bytes are recorded separately and may coexist with residual state or cleanup failure. Cleanup finishes boundedly before return, never continues as hidden background work, and never overwrites the operation's primary failure.

**Compilation Operation Outcome**:
The typed non-semantic outcome of running a Prepared Compilation before Compilation Terminal Commitment. Its closed classes cover dependency resolution and acquisition, dynamic resource limits, deadline, cancellation, supersession, execution or isolation failure, and internal integrity failure, with the terminal phase and structured cause retained. Statically measurable request-limit violations are instead aggregated Compilation Request Rejections. It exposes no universal retryable flag; callers derive retry policy from the cause and their context. Deterministic compiler or exporter rejection from accepted exact inputs is a Compilation Result, while dynamic limit, cancellation, process, or infrastructure failure during the same phase remains operational. An operation outcome retains the Compilation Request Inventory and dependency evidence accumulated before it terminated. Failures after terminal commitment are Compilation Delivery Outcomes or other adapter outcomes that refer to, but never replace, the immutable Compilation Report.

**Canonical Compilation Diagnostic**:
A structured compiler or exporter diagnostic identified by phase, severity, kind, logical spans, message, hints, and stable semantic ordering. Compiler diagnostics precede exporter diagnostics; deterministic engine order is preserved within each phase, and library-generated diagnostics have specified semantic positions. The phase, severity, and span envelope is stable across engine upgrades; kinds are library-defined where a real stable category exists and otherwise retain namespaced engine-specific detail scoped by exact engine identity. Warnings and errors share this one canonical collection. Messages, hints, and source-derived values remain untrusted and potentially sensitive presentation data; canonical structure does not make them terminal-safe, markup-safe, or publisher-authenticated. Terminal rendering, color, absolute host paths, operational explanations, and other adapter presentation are not part of it.

**Canonical Diagnostic Policy**:
The explicit versioned semantic policy that bounds Canonical Compilation Diagnostics by retained entry count and aggregate canonical encoded entry bytes. The effective policy is fixed during preparation, recorded in the Compilation Request Inventory and Engine-Neutral Compilation Intent, and contributes to Compilation Identity. Adapter profiles may resolve documented defaults, but changing or tightening this policy creates a different semantic request; it is not an Operation Resource Limit.

**Canonical Diagnostic Envelope**:
The identity-bearing ordered diagnostic value owned by one Compilation Result under one Canonical Diagnostic Policy. It retains the maximal canonical-order prefix of whole Canonical Compilation Diagnostics that fits both policy dimensions and ends in either Complete or a typed Limited record naming the first omitted ordinal, phase, and limiting dimension. Messages, hints, spans, and other diagnostic fields are never partially truncated, the fixed-size completion record is outside the encoded-entry byte budget, and no omitted total is required. The policy, retained entries, and completion record contribute to Compilation Result Identity; inability to construct this envelope within separately admitted operational resources produces a Compilation Operation Outcome rather than an incomplete result.

**Diagnostic Projection**:
A non-semantic disclosure or rendering derived from a Compilation Terminal, Compilation Report, or Canonical Diagnostic Envelope. It may redact exact text, limit entries or rendered bytes, escape content for its target, and add separately authorized source context, but it reports its projection status and originating identity and never substitutes for the canonical value. Projection limits and failures do not change Compilation Identity, Compilation Result Identity, result status, or the underlying report.

**Diagnostic Source Bundle**:
Optional owned, deduplicated exact effective source bytes and logical bindings for sources referenced by diagnostics retained in the Canonical Diagnostic Envelope, retained under an explicit operational reporting policy so adapters can render rich diagnostics after compilation returns. An overridden source binds to its effective Pack Override bytes without adding baseline bytes. Generic library execution does not request this bundle by default. It is sensitive, excluded from compilation identities, and not required for machine-readable diagnostics. Logical binding count, unique blob count, largest blob, aggregate exact source bytes, and metadata bytes have independent explicit limits; its channel distinguishes not requested, complete, limited, and unavailable collection. A cache hit remains valid when requested source context is absent and reports the channel unavailable rather than forcing re-execution.

**Compilation Timing Telemetry**:
Optional non-semantic measurements from the explicit monotonic time domain used by one lifecycle operation. Generic library execution does not request them by default, although a Compilation Attempt Deadline uses the same time domain whenever present. Preparation telemetry accompanies its Prepared Compilation or Compilation Request Rejection; a Compilation Report may report stable admission and queueing, acquisition, verification and spooling, kernel compilation, export, and finalization phases, whose parallel durations need not sum to elapsed time. Compilation Delivery reports timing separately. Finer engine timing is explicitly opt-in, best-effort, and engine-specific; it is complete for one attempt only when its Engine Runtime Domain can exclude overlapping engine work, and is otherwise limited or unavailable. Each reporting channel distinguishes not requested, complete, limited, and unavailable; instrumentation failure or a cache entry without historical timing does not change the semantic result or force re-execution.

**Logical Dependency Identity**:
The canonical semantic address of a dependency, independent of where its bytes were acquired. A successful dependency observation pairs this logical identity with exact content identity; backing source identity remains Dependency Resolution Evidence rather than semantic content identity.

**Dependency Resolution Evidence**:
The canonical set of revalidatable facts that determined how a logical dependency request resolved, including content selection, authority choice, every higher-priority miss that enabled fallback, final logical missing outcomes, and relevant absence or membership. A fact is included when changing it while holding the compilation request fixed could change resolution, diagnostics, success, or artifacts. Access order, repeat counts, retries, and other causally irrelevant attempts are not semantic. A failed compilation retains the evidence accumulated through its terminal failure.

**Dependency Evidence Key**:
A sanitized stable identity and immutable version, content, absence, or membership fact supplied by an authority for Dependency Resolution Evidence. It can be revalidated or subscribed to without making credentials, absolute host paths, or transport details part of semantic identity. A backing-source notification dirties the evidence; equal revalidation preserves semantic identities and caches. Full keys live in operation results and active sessions; Packs retain sanitized creation provenance rather than source-specific revalidation handles.

**Dependency Resolution Cache**:
An authority-owned operational facility that retains one complete source-selection or failure outcome together with all causal Dependency Resolution Evidence within the same Cache Isolation Domain and authority composition. Its mutable resolution records are lookup hints rather than authority of truth: every dirty fact must be revalidated, equal revalidation preserves the record, and failed or incomplete revalidation leaves it unusable. Transient acquisition, cancellation, deadline, and incomplete outcomes are never entries.

**Backing Dependency Locator**:
An optional adapter projection that links a Dependency Evidence Key to a physical filesystem path, URL, object key, or equivalent location for watch and build-tool integration. It may be sensitive, is not portable, and does not enter Pack state, Compilation Identity, or Compilation Result Identity.

**Dependency Acquisition Outcome**:
The terminal typed outcome from one authority source whose result affected dependency resolution: success, unavailable or missing, transient failure, permanent failure, invalid content, or integrity mismatch. Only unavailable or missing permits fallback; authorization denial is a permanent failure and never masquerades as absence. Internal retries, redirects, timing, and cache mechanics are operation telemetry rather than canonical outcomes.

**Compilation Access Trace**:
The canonical set of logical project, package, and font requests Typst actually observed during one compilation, with request kind, outcome, effective content provenance, and links to the Dependency Resolution Evidence that determined each outcome. Successful project observations identify the project path and request kind, then use Pack-contained identity for baseline bytes or the request's role-bound Compilation Request Commitment for override bytes; package observations identify the Package Requirement, package-relative path, exact file bytes, and embedded or external fulfillment; font observations identify the Font Container, face index, and embedded or external fulfillment. Pack creation origin and sanitized acquisition provenance remain owned by the Pack rather than repeated here. It is distinct from the Compilation Request Inventory; access order and repeat counts are not semantic. A failed compilation retains the observations accumulated through its terminal failure.

**Reproducibility Claim**:
A normative, testable relation from one baseline Compilation Result to the result expected under a named reproducibility or engine-compatibility guarantee. One baseline is enough to state the claim; repeated matching execution may support it but is not required to create it. Changed identity-bearing inputs form a different claim, and later backing-source changes or unavailability do not revoke the historical claim, while a mismatching semantic result refutes it.

**Exact Reproducibility Claim**:
A Reproducibility Claim that every Compilation Result produced for one Compilation Identity has the baseline Compilation Result Identity. Engine and exporter platform qualification is already carried by that Compilation Identity. Compilation Operation Outcomes do not compete with or refute the claim unless a separate operational guarantee promised their absence.

**Cross-Engine Compatibility Claim**:
A Reproducibility Claim comparing distinct Compilation Identities derived from one Engine-Neutral Compilation Intent under named source and target Engine Identities, Exporter Identities, scope, and Cross-Engine Compatibility Level. It never makes the engine-specific Compilation Identities or Compilation Result Identities equal and never extends beyond its explicit compilation intent.

**Cross-Engine Compatibility Level**:
One of four cumulative, testable strengths. Request Compatible means both implementations can prepare the same fully explicit Engine-Neutral Compilation Intent. Closure Compatible additionally means the target reaches a Compilation Result using only the Pack contract and exact declared fulfillments, without undeclared fallback. Structurally Compatible additionally requires equal result status, Compilation Document Summary, Canonical Diagnostic Envelope structure and completion state, semantic dependency projection, and ordered artifact roles, but not equal artifact bytes, diagnostic wording or hints, or engine-specific diagnostic detail. Exactly Reproducible additionally requires equality of the complete engine-neutral result projection, including the complete Canonical Diagnostic Envelope and exact artifact bytes.

**Pack Override**:
A compilation-scoped byte replacement for any project file contained in a Pack, including Typst source and the fixed entrypoint. It cannot add or delete a path, change the entrypoint identity, or replace package or font content.
_Avoid_: placeholder, replacement asset

**Pack Override Set**:
The immutable, finite set of Pack Overrides owned by one exact compilation request and validated against one Pack Identity. Each canonical contained project path appears at most once, declaration order is not semantic, and compilation observes one fixed path-to-exact-bytes snapshot rather than consulting an override provider. Replacement bytes remain opaque unless requested as Typst source. Every member contributes to Compilation Identity through its canonical path, exact size, and role-bound Compilation Request Commitment even when its bytes are not read; observed use is separate provenance. Replacement Content Identity, baseline-comparison identity, and byte-identical status are available only to authorized internal or sensitive projections. The set is semantic request data rather than an authorization grant; an adapter authorizes who may supply it separately.

**Compilation Session**:
A caller-owned, Pack-bound coordinator for repeatedly evaluating changing compilation request state. It owns the pure preparation of each stable request under that revision's exact preparation limits, revision and evaluation ordering, dependency currentness, one active or draining attempt plus one latest pending revision, publication, Last Successful Compilation, and retirement. Authorities, caches, watchers, clocks, runtimes, execution facilities, and delivery remain explicit caller facilities. A different Pack Identity starts a different Compilation Session with a distinct session-instance identity.

**Compilation Session Revision**:
The session-local monotonic identity of one accepted desired observation. A revision owns either an immutable semantic request snapshot or a Session Ingestion Failure, its exact effective session policy including requested and admitted preparation limits, and the dependency dirtiness that must be reconciled before it can be current. An effective request, preparation limit or policy change, dependency invalidation, or explicit refresh creates a new revision; an operational retry remains within its revision and creates a new Session Evaluation. Revisions are operational, do not contribute to Compilation Identity, and may resolve to the same Compilation Identity or Compilation Result as another revision.

**Session Evaluation**:
One session-ordered evaluation of a Compilation Session Revision, distinguishing the initial evaluation from operational retries within that revision. It yields exactly one candidate terminal observation: the revision's Session Ingestion Failure, a Compilation Request Rejection from pure preparation, or a Compilation Report from one Compilation Attempt. Session Evaluation identity is operational and orders same-revision races without changing Compilation Identity.

**Session Ingestion Failure**:
The typed adapter-owned terminal for an accepted desired observation that could not be stabilized into one coherent immutable semantic request. It identifies the failed request-source scopes and owns no Prepared Compilation, Compilation Identity, Compilation Attempt, Compilation Report, dependency evidence, or access trace. It may be published only with stale or unverified Session Currentness and never receives a Compilation Attempt token.

**Superseded Session Attempt**:
A physically outstanding Compilation Attempt whose Session Evaluation has irreversibly lost publication eligibility to a newer accepted revision or evaluation. It remains session-owned until it returns or is forcibly reaped: supersession recorded before Compilation Terminal Commitment produces the corresponding Compilation Operation Outcome, while an already committed report remains an immutable historical value that cannot later win Session Publication.

**Dependency Change Notification**:
A non-semantic hint that one Dependency Evidence Key or a declared provider scope may no longer describe current backing state. It never supplies trusted replacement evidence: the affected facts must be revalidated. Duplicate, reordered, or coarse notifications may be coalesced, while a detected delivery gap dirties the complete affected scope.

**Session Watch Coverage**:
The explicit operational claim describing whether every mutable declared request source and every causal Dependency Evidence Key for a Compilation Session is immutable or covered by race-free notification and revalidation. Complete push coverage detects notification loss and closes read-to-subscribe gaps; complete polling coverage guarantees convergence after a coherent poll once relevant state quiesces. Incomplete coverage identifies the uncovered scopes and never presents a result as known current for them.

**Session Currentness**:
The explicit operational relation between a published terminal evaluation and the mutable state watched by a Compilation Session. Healthy complete push coverage may claim reconciliation through its latest provider cursors; complete polling coverage claims only the state observed by its last coherent poll; incomplete coverage remains unverified. An accepted change, dirty notification, notification gap, or coverage downgrade makes the prior publication stale until the required revalidation succeeds. A Semantic Result Cache hit supplies only a historical result candidate: currentness requires fresh stabilization of mutable request sources, revalidation or reacquisition of every causal dependency fact, and race-free watch handoff. Currentness never changes the validity of an immutable historical Compilation Result.

**Session Publication**:
The atomic session-owned transition that exposes a Compilation Terminal or Session Ingestion Failure together with its Session Currentness for one Session Evaluation. Every publication is fenced by its session-instance identity and a monotonic publication sequence. Only the latest desired revision and evaluation may publish; the transition rechecks their eligibility, live evidence, dirtiness, coverage, and installed subscription generation before committing. A cache-hit result retains its original Compilation Access Trace while reconstructed live evidence remains publication metadata. A report committed before later supersession remains an immutable historical value but cannot publish afterward. Notification of a committed publication is an output effect rather than a second state transition; Session Publication is distinct from progress reporting, Compilation Terminal Commitment, and Compilation Delivery.

**Last Successful Compilation**:
The most recent succeeded Compilation Result that won Session Publication. It remains one immutable whole result and artifact set, retains its originating session revision and result identity, and is explicitly stale whenever it is not current, including after a notification gap or watch-coverage downgrade. Rejection, operational failure, supersession, or delivery failure does not replace it; last successful delivery is separate adapter state.

**Session Retirement**:
The irreversible shutdown of one Compilation Session. Retirement rejects new desired observations and retries, revokes every unpublished candidate, retires active and proposed subscription generations, and records supersession for any active attempt before requesting its interruption. The session is retiring while an attempt or subscription-arm operation can still produce owned work and is retired only after that work returns, is reaped, or is abandoned with a guarantee that no live resource remains. Late completions cannot publish, and the retained Last Successful Compilation is stale historical state.

**Session Recovery Record**:
A caller-owned durable application record used to seed a new Compilation Session after interruption. It binds an application session key and fencing generation to the exact Pack Identity, reconstructible desired request values, effective policy, and optional verified historical result references. It never resumes attempts, revisions, subscriptions, facilities, or currentness from the prior process; restoration creates a new session instance and requires fresh ingestion, evidence reconciliation, and race-free subscription before Session Publication can claim currentness.
