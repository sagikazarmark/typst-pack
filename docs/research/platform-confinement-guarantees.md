# Platform Confinement Guarantees by Deployment Target

Research for [issue 53](https://github.com/sagikazarmark/typst-pack/issues/53), 2026-07-18.

## Decision

A Deployment Trust Profile is supportable only when the deployment can establish and verify every property that the profile requires. A library feature flag, worker protocol, container label, or WebAssembly binary is not an enforcement boundary by itself.

The target decisions are:

- Ordinary in-process native library use supports Trusted and Partially Trusted operations. It must refuse Hostile operations.
- A native CLI supports Hostile operations only when a trusted launcher establishes a verified operating-system sandbox or virtual machine before any complex input interpretation. The ordinary CLI must refuse Hostile operations.
- An ordinary OCI or Dagger container supports Trusted and Partially Trusted operations. It must refuse Hostile operations unless the deployment separately verifies a complete hardened shared-kernel sandbox or a VM/microVM runtime, all required quotas, deny-by-default capabilities, and parent-side verification and publication.
- An ordinary in-process or ordinary-container multi-tenant service must refuse Hostile operations. A service supports Hostile operations only through per-attempt confined workers and a trusted parent that verifies the terminal protocol before publication.
- A browser main thread and a standard browser Dedicated Worker, including one running a fixed WebAssembly guest, must refuse Hostile operations. A Dedicated Worker plus fixed WebAssembly is useful Partially Trusted isolation, but Web standards expose neither a hard whole-worker CPU and memory contract nor a verifiable renderer containment boundary.
- A server-side WebAssembly/WASI runtime can support Hostile operations only conditionally: imports must be deny-by-default, guest work must be metered and interruptible, and an outer process or VM must enforce total memory, blocking host calls, deadline, forced termination, and reaping.

The existing Isolated Compilation Worker is not sufficient for Hostile input. Its current seam begins after Pack and dependency interpretation. Hostile support needs a broader hostile-operation boundary that starts before Pack Archive, Pack Control Record, package, font, source, compiler, or exporter parsing. The existing Compilation Kernel and worker protocol may remain inside that broader boundary.

## Governing Contract

This paper applies the definitions resolved in [issue 47](https://github.com/sagikazarmark/typst-pack/issues/47) and recorded in `CONTEXT.md`:

- Trusted permits defensive validation but makes no adversarial containment, process-survival, prompt-termination, or hard whole-operation CPU and retained-memory claim.
- Partially Trusted treats content as externally controlled and potentially abusive while trusting deployment code and executable facilities. In-process interpretation is permitted, but it makes no same-process compromise, crash-survival, or hard whole-operation resource claim.
- Hostile includes deliberate attacks on parsers, the compiler, exporters, protocols, resource controls, and confinement. Containment may be claimed only for an explicitly reported scope enforced by the operating system or runtime before complex interpretation. There is no in-process fallback or silent downgrade.

Profiles do not alter semantic or integrity validation. They are selected independently for each operation, before input-dependent interpretation, and never become Pack or identity state. If any admitted input requires a stronger profile, that profile governs the operation.

[Issue 46](https://github.com/sagikazarmark/typst-pack/issues/46) additionally fixes one synchronous Compilation Kernel over an immutable, verified Compilation Dependency Snapshot. It states that process isolation provides killability and resource placement but is not, by itself, a hostile-input sandbox. This paper preserves that distinction.

## Support Vocabulary

The matrix uses three outcomes:

- **Supported**: the target's normal contract can honestly provide the profile.
- **Conditional**: the target may advertise the profile only after admission verifies every named deployment prerequisite. A missing or unverifiable prerequisite is a refusal, not weaker execution.
- **Refuse**: the target cannot provide the profile under the described execution shape and must fail admission before complex interpretation.

## Target Matrix

| Target and execution shape | Trusted | Partially Trusted | Hostile | Required Hostile admission result |
| --- | --- | --- | --- | --- |
| Native library in the caller's process | Supported | Supported, with explicit same-process and best-effort resource caveats | Refuse | Direct library execution never advertises Hostile |
| Native CLI in its ordinary process | Supported | Supported | Refuse | Require an isolated launcher/worker mode |
| Native CLI with a verified OS sandbox or VM established before parsing | Supported | Supported | Conditional | Admit only the platform policies described below |
| Ordinary OCI container | Supported | Supported | Refuse | A container name, namespace set, or default profile is insufficient |
| OCI/Dagger execution under a verified hardened runtime or microVM | Supported | Supported | Conditional | Verify runtime class, policy, quotas, capabilities, network, mounts, handles, kill, reap, protocol, and publication |
| Service compiling in the request process | Supported | Supported | Refuse | Route Hostile work to an isolated worker or reject it |
| Service with per-attempt confined workers and trusted supervision | Supported | Supported | Conditional | The worker boundary must begin before complex parsing |
| Browser main thread | Supported | Supported only within the page's availability tolerance | Refuse | Never run Hostile work on the main thread |
| Browser Dedicated Worker with a fixed WebAssembly guest | Supported | Supported with bounded inputs, fixed guest imports, termination, and bounded outputs | Refuse | Standard browser APIs do not meet the complete Hostile resource and containment contract |
| Server-side WebAssembly/WASI in the service process only | Supported | Supported | Refuse | Runtime limits alone do not bound all host/runtime resources or contain runtime compromise |
| Server-side WebAssembly/WASI inside an OS-confined process or VM | Supported | Supported | Conditional | Verify both the Wasm capability/metering policy and the outer process policy |

"Supported" for Partially Trusted never implies hard interruption, retained-memory containment, or survival after an in-process implementation defect. Deployments may still choose stronger isolation.

## Hostile Operation Shape

The trusted outer layer must be intentionally small. It may authenticate the caller, choose policy, perform bounded admission, count and hash raw bytes, spool them into stable enforcement-owned storage, construct the sandbox, supervise it, verify a bounded terminal response, and publish verified artifacts. Hashing hostile bytes does not make them trusted.

```text
caller or authority
        |
        v
bounded ingress, byte count, hash, stable spool          trusted parent
        |
        v
immutable bytes or narrowly scoped read-only handles
        |
        v
+--------------------------------------------------+
| hostile boundary                                 |
| Pack framing and decompression                   |
| Pack Control Record decoding and Pack validation |
| package, font, override, and source validation   |
| preparation, Typst execution, and export         |
| bounded terminal worker protocol production      |
+--------------------------------------------------+
        |
        v
bounded framing and semantic-envelope verification         trusted parent
        |
        v
private staging and atomic publication where supported     trusted parent
```

### Stage Placement

| Stage | Hostile placement | Reason |
| --- | --- | --- |
| Profile selection and capability appraisal | Trusted parent, before input interpretation | A worker cannot choose or weaken its own containment contract |
| Authentication, authorization, credentials, and network acquisition | Trusted parent or separately confined acquisition service | Credentials and broad network authority must not enter the hostile worker |
| Raw transport byte count, bounded spool, and cryptographic hash | Trusted parent, using simple bounded code | These establish stable bytes without claiming semantic validity |
| Pack Archive framing, range planning, decompression, and entry interpretation | Inside hostile boundary | Archive parsers and decompressors are deliberate attack targets |
| Pack Control Record decoding, schema validation, and whole-Pack construction | Inside hostile boundary | Canonical CBOR and semantic validators are complex hostile-byte interpreters |
| Package tree and `typst.toml` validation | Inside hostile boundary | Package names, manifests, paths, and bytes are hostile inputs |
| Font container parsing, face validation, and catalog derivation | Inside hostile boundary | Font parsers are part of the hostile attack surface |
| Pack Override ingestion and semantic request preparation | Inside hostile boundary | Replacement bytes and values may reach Typst or another parser |
| Typst source/data interpretation, compilation, and all exporters | Inside hostile boundary | Hostile explicitly includes attacks on the compiler and exporters |
| Temporary expansion, compiler scratch space, and output staging | Inside quota-backed worker storage | The worker must not receive a host destination or unbounded temporary directory |
| Worker protocol parsing and verification | Trusted parent, with a small bounded parser | Worker compromise makes every returned byte hostile |
| Destination selection, filesystem projection, and Compilation Delivery | Trusted parent after verification | Hostile data must not choose host paths or publish partial output |
| Terminal, log, HTML, or JSON rendering | Final trusted presentation boundary | Diagnostics and worker messages remain untrusted strings |

The parent may repeat a hash or simple length check, but Hostile correctness cannot depend on semantic validation performed before confinement. The exact bytes validated inside the boundary must be the bytes later executed. A mutable path that is checked and reopened is not sufficient.

### Input Handoff and TOCTOU

The portable rule is to copy each admitted input into an enforcement-owned immutable value and hand only that value into the worker. Read-only access is not the same as immutability when another principal can still modify the backing object.

On Linux, a sealed `memfd` is a suitable handoff after the parent has finished writing it: `memfd_create` supports file seals specifically to prevent an untrusted peer from changing or shrinking shared bytes and causing TOCTOU or `SIGBUS` behavior ([`memfd_create(2)`](https://man7.org/linux/man-pages/man2/memfd_create.2.html)). A filesystem adapter can resolve beneath a captured directory file descriptor with `openat2` and `RESOLVE_BENEATH` or `RESOLVE_IN_ROOT`, explicitly deny magic links, and use the resulting handle rather than reopen a string path ([`openat2(2)`](https://man7.org/linux/man-pages/man2/openat2.2.html)).

On Windows, the launcher must pass only an explicit handle list; inherited handles refer to the same objects with the same access rights, so broad or accidental inheritance is authority leakage ([Windows process inheritance](https://learn.microsoft.com/en-us/windows/win32/procthread/inheritance)).

In a browser, `Blob` represents immutable raw binary data and serialized `Blob` values retain their underlying byte sequence, while disk-backed `File` snapshot behavior is only a `should` requirement. A browser adapter should therefore complete a bounded read and transfer or copy the resulting buffer to the worker before interpretation rather than rely on a mutable external file reference ([File API](https://w3c.github.io/FileAPI/)).

## Trusted-Computing Boundaries

Hostile execution has two different trust questions that reports must not conflate.

### Containment TCB

The containment trusted-computing base consists of:

- the profile admission and policy-selection code;
- the bounded ingress and immutable handoff mechanism;
- the launcher, supervisor, deadline monitor, and process reaper;
- the OS kernel, hypervisor, or language runtime that enforces the admitted boundary;
- the exact sandbox policy and deployment control plane that selects it;
- the bounded parent-side worker protocol verifier; and
- the private staging and publication adapter.

Pack readers, decompressors, package and font validators, Typst, exporters, and worker protocol producer are not trusted for containment. They are placed inside the boundary because Hostile assumes one of them may be compromised.

### Semantic-Correctness TCB

The same parsers, validators, compiler, and exporters remain trusted for semantic honesty. If hostile input fully compromises the worker, parent-side framing, length, identity, role, and terminal-completeness checks can prevent unauthorized host effects and malformed publication, but they cannot prove that a forged Compilation Result is semantically correct. Hostile is a containment claim, not proof-carrying computation.

Output consumers are also outside the claim. A valid artifact can still attack a later PDF, SVG, image, HTML, terminal, or archive consumer.

## Native OS Mechanisms

### Linux

A Linux native CLI or service worker may conditionally admit Hostile operations when the deployment accepts the host kernel as part of the containment TCB and verifies a complete policy before starting the worker. The policy needs all of the following, not one mechanism in isolation:

- Start the worker directly in a dedicated cgroup v2 subtree before it executes hostile code. Configure and verify `memory.max`, `pids.max`, CPU bandwidth, relevant I/O controls, and the parent deadline. `cgroup.kill` kills the complete descendant tree with `SIGKILL`; `cgroup.events` can establish when it is no longer populated. The kernel documents that controllers are not enabled by default, so their mere availability is not enough ([cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)).
- Use the CPU controller as a bandwidth ceiling together with an absolute monotonic deadline and forced termination. `cpu.max` is a quota per period, not a total operation-work counter. Any reported hard CPU bound must state accounting granularity and maximum enforcement overshoot.
- Construct a minimal mount namespace with a read-only runtime image, immutable input handles, a private size-limited temporary filesystem, and no host destination, cache, credential, socket, device, or control-plane mount.
- Use distinct user, PID, IPC, UTS, mount, and network namespaces as applicable; drop every capability; set `no_new_privs`; deny process creation unless explicitly required; and close every unrelated file descriptor. `no_new_privs` persists across `fork`, `clone`, and `execve`, but does not itself prevent privilege changes unrelated to `execve` ([Linux `no_new_privs`](https://docs.kernel.org/userspace-api/no_new_privs.html)).
- Install an architecture-aware seccomp allowlist before hostile interpretation. The Linux documentation explicitly says seccomp filtering is not a sandbox; it reduces kernel surface and must be combined with other hardening and access-control mechanisms ([seccomp BPF](https://docs.kernel.org/userspace-api/seccomp_filter.html)).
- Apply a verified deny-by-default filesystem policy through the mount graph and an LSM such as Landlock, AppArmor, or SELinux. Landlock can restrict ambient filesystem and network rights, but supported rights vary by ABI and descriptors opened before restriction retain authority. Hostile admission must fail when the policy depends on an unavailable right; the documentation's best-effort compatibility pattern is not sufficient for this profile ([Landlock](https://docs.kernel.org/userspace-api/landlock.html)).
- Deny network structurally, normally with a network namespace containing no usable interface and a socket-denying seccomp policy. A contractually unused network API is not enforced denial.
- Use a quota-backed temporary filesystem or volume. A directory inside a mount namespace is not a storage quota.
- On deadline, cancellation, protocol violation, or quota event, kill the complete cgroup, wait until it is empty, reap the worker, and discard all staging.

If the deployment does not accept a shared host kernel as the intended Hostile boundary, a Linux namespace/container policy must refuse Hostile and use a separately trusted application-kernel sandbox or VM/microVM instead. gVisor explains that ordinary Linux primitives still leave the workload one system call away from the shared host kernel, while its Sentry reimplements the guest system-call interface and uses host primitives as defense in depth ([gVisor introduction](https://gvisor.dev/docs/architecture_guide/intro/)). Firecracker places malicious guest vCPUs behind KVM and the VMM, then additionally constrains the VMM with seccomp, cgroups, namespaces, and a privilege-dropping jailer ([Firecracker design](https://github.com/firecracker-microvm/firecracker/blob/main/docs/design.md)). These reduce different attack surfaces; neither name substitutes for verification of the complete typst-pack policy.

### Windows

A Windows native CLI or service worker may conditionally admit Hostile operations with this combined boundary:

- Launch a dedicated worker under an AppContainer profile with no capabilities except the exact resources required. AppContainer blocks device resources by default and scopes file, registry, network, process, credential, and window access; granted capabilities define the remaining authority ([AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)).
- Use a restricted token as defense in depth to remove privileges, mark SIDs deny-only, and add restricting SIDs. A restricted token alone is not a complete sandbox, and Microsoft warns that a restricted application on the default desktop can attack unrestricted applications through window messages ([restricted tokens](https://learn.microsoft.com/en-us/windows/win32/secauthz/restricted-tokens)).
- Create the process suspended, place it in a Job Object before it executes worker code, prohibit breakaway, and configure process-tree, CPU-time, active-process, and memory limits. Job Objects manage associated processes as a unit, normally include child processes, can enforce limits, and support kill-on-close ([Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)). Job-wide committed-memory limits are distinct from per-process limits ([extended job limits](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information)).
- Pass only the protocol and immutable input handles through an explicit inherited-handle list. Do not inherit the environment, current directory, console, standard handles, or ambient file mappings without an explicit need.
- Deny network capabilities and broker access, use private quota-backed temporary storage, and retain destination and publication handles only in the parent.
- On interruption, call `TerminateJobObject`, wait for all job processes, and discard staging. Associated processes cannot postpone or handle job termination ([`TerminateJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject)).

The admitted scope must identify the AppContainer capabilities, token restrictions, Job Object limits, inherited handles, and temporary-storage quota. If those settings cannot be queried and verified, or if the Windows kernel is not an accepted containment TCB, the target must refuse Hostile and use an externally enforced VM boundary.

### macOS

App Sandbox restricts a macOS application's access to files, network connections, hardware, system resources, and user data through entitlements ([Apple App Sandbox](https://docs.developer.apple.com/tutorials/data/documentation/security/app-sandbox.md)). It is useful for Partially Trusted defense in depth and for reducing a worker's authority.

App Sandbox alone does not expose the complete typst-pack Hostile contract: the cited public contract does not supply a hard whole-operation CPU budget, total process-tree memory and temporary-storage quota, or a complete kill-and-reap policy. A generic macOS CLI therefore must refuse Hostile. A macOS deployment may advertise Hostile only when an external VM or other separately verified runtime supplies every generic Hostile property, including the missing resource and lifecycle controls, before interpretation.

## Containers and Dagger

### Why an Ordinary Container Is Not Enough

Docker uses namespaces for isolation and cgroups for accounting and limits, but its own security documentation identifies the daemon, custom configuration, capabilities, mounts, and the shared kernel as security concerns. It warns that a host directory can be shared without limiting the container's access rights and that default capabilities and mounts may provide incomplete isolation, especially with kernel vulnerabilities ([Docker Engine security](https://docs.docker.com/engine/security/)).

Defaults also fail the Hostile resource contract. Docker states that containers have no resource constraints by default. Memory and CPU constraints must be configured explicitly; CPU quota is a scheduler bandwidth ceiling, and some settings are soft rather than hard ([Docker resource constraints](https://docs.docker.com/engine/containers/resource_constraints/)). Docker's default seccomp policy is intentionally only "moderately protective" for broad application compatibility ([Docker seccomp](https://docs.docker.com/engine/security/seccomp/)).

Consequently, `docker run`, an OCI image, or an ordinary Kubernetes Pod is not evidence of Hostile support.

### Dagger

Dagger documents that Functions do not receive host filesystems, services, sockets, secrets, or other host resources unless the top-level call grants typed arguments ([Dagger sandboxed runtime](https://docs.dagger.io/features/sandbox/)). This is a valuable least-authority adapter rule, but it is not a complete claim about hostile parser containment, shared-kernel escape, hard quotas, or worker protocol verification.

The Dagger `Container` is OCI-compatible and can copy or mount directories and files, mount caches, attach registry authentication, bind services, open an interactive terminal, export to the host, or publish to a registry ([Dagger Container](https://docs.dagger.io/getting-started/types/container/)). Dagger services explicitly enable container-to-container, container-to-host, and host-to-container networking when bound ([Dagger services](https://docs.dagger.io/features/services/)). Each such grant changes the available authority.

The ordinary Dagger function must therefore refuse Hostile. A Dagger deployment may conditionally admit it only if the operator verifies all of these facts outside the function:

- the actual execution runtime is a separately accepted hardened sandbox such as gVisor, or a VM/microVM boundary such as Firecracker, rather than an assumed ordinary OCI runtime;
- the worker has no Secret, Socket, Service, cache, registry credential, host directory, host file, interactive terminal, privileged device, host namespace, or publication capability;
- input is a stable immutable Dagger value or is copied into one before worker parsing, and is exposed read-only;
- memory, CPU bandwidth and deadline, process count, I/O, temporary storage, and output bytes have actual enforced limits;
- network is structurally denied, not merely unused;
- all capabilities are dropped, privilege escalation is disabled, and a narrow seccomp/LSM policy is active where relevant;
- forced termination kills and reaps the complete runtime scope; and
- the Dagger caller receives only a bounded response for parent-side verification and performs host export or publication afterward.

If the Dagger API or engine cannot inspect and prove these properties for the active deployment, `Hostile` is unavailable. The function must not infer it from Dagger's general "sandboxed" wording.

## Multi-Tenant Services

An in-process service has the same refusal as the native library. An ordinary container-per-request service has the same refusal as the ordinary container. A multi-tenant service may conditionally support Hostile only through this architecture:

- The trusted request tier authenticates and authorizes, selects Hostile before content parsing, enforces request-body and queue admission limits, and acquires external bytes without giving credentials to the worker.
- Every attempt receives a fresh or provably reset isolation scope. Different tenants and concurrently hostile attempts never share a writable filesystem, runtime Store, process, temporary directory, output buffer, or publication capability.
- The platform scheduler establishes quotas and deny-by-default network, filesystem, process, handle, IPC, and device policy before worker execution. The worker cannot select its runtime class or weaken its policy.
- The complete parsing-through-export pipeline runs inside that scope. The existing Compilation Kernel can remain the inner semantic path.
- A trusted supervisor owns the deadline, cancellation race, forced termination, complete-tree reaping, and staging cleanup. No hidden work survives the attempt's return.
- The trusted parent accepts one bounded, versioned terminal response; independently verifies framing, implementation identities, Compilation Identity, result status, diagnostic and artifact counts, artifact roles, lengths, content identities, and terminal completeness; and rejects trailing, duplicate, or inconsistent material.
- Only the parent maps verified artifact roles to an authorized destination and performs atomic publication where the sink permits.

The service control plane, worker image selection, sandbox policy distribution, node/runtime selection, queue, and parent verifier are part of the containment TCB. An untrusted plugin or authority must have its own boundary; a Rust trait, service account, or container sidecar does not make executable deployment code trusted.

## Browser and WebAssembly

### Browser Main Thread

The main thread must refuse Hostile. Non-cooperative compilation can block the page's event loop, cannot be forcibly separated from page state, and has no per-operation hard memory or CPU quota. It is suitable only for Trusted or explicitly caveated Partially Trusted operations.

### Dedicated Worker

A Dedicated Worker improves Partially Trusted behavior by running independently of UI scripts and communicating through structured messages. The creator can call `terminate()`, after which the worker's active event loop is aborted and queued tasks are discarded ([HTML workers](https://html.spec.whatwg.org/multipage/workers.html#dom-worker-terminate)). A safe browser adapter should use a fresh Dedicated Worker per attempt, transfer immutable input bytes, bound every message, terminate on deadline or cancellation, and expose no result until a complete response verifies.

This still does not meet Hostile:

- Workers can use host APIs exposed by their global scope, including script import and network APIs, unless application design and browser policy remove them. A Worker is not a deny-by-default capability environment.
- The Web standard defines worker termination but no hard per-worker CPU accounting, retained-memory maximum, temporary-storage maximum, process-tree concept, or maximum termination latency.
- Browser storage quota is an implementation-defined conservative estimate and usage is only a rough estimate, not an operation-scoped hard temporary-storage reservation ([Storage Standard](https://storage.spec.whatwg.org/)).
- The Web API does not expose or attest whether a worker has a separate OS process, which renderer sandbox is active, or whether compromise is contained from same-origin application data.

### WebAssembly in a Browser Worker

WebAssembly strengthens the Partially Trusted design when the application loads one fixed trusted typst-pack module and treats only Pack and compilation bytes as hostile. WebAssembly linear memories are isolated from runtime memory, accesses are bounds checked at the memory-region level, and modules interact with the outside world only through imports supplied by the embedder ([WebAssembly security](https://webassembly.org/docs/security/)). The JavaScript embedding explicitly obtains functions, memories, tables, globals, and tags from the import object, so each import is authority ([WebAssembly JavaScript interface](https://webassembly.github.io/spec/js-api/)).

The browser adapter should instantiate a fixed module with a minimal audited import object, no general WASI polyfill, no dynamic code or script import, no shared memory, fixed memory and table maxima, a bounded input/output ABI, and all Pack parsing, dependency validation, Typst execution, and export inside the guest. The main thread remains the supervisor and response verifier.

Even that shape must refuse Hostile. A WebAssembly maximum bounds guest linear memory, not JavaScript objects, JIT/compiler metadata, native stacks, runtime bookkeeping, messages, or browser process memory. Standard browser WebAssembly has no fuel or epoch API, and Worker termination does not provide a verifiable hard CPU or renderer-containment contract. A managed browser embedded inside an externally quota-controlled OS process or VM could be evaluated as a native isolated service, but that is not a portable browser-target guarantee.

A browser can still offer Hostile behavior by sending immutable bytes to a remote Hostile-capable service. In that design the service, not the browser worker, owns the Hostile claim.

## Server-Side WebAssembly and WASI

Server-side WebAssembly can form the inner runtime boundary for Hostile work. Wasmtime states that WebAssembly instances have no raw system-call or I/O access and can interact with the outside world only through explicitly linked imports; its safe API and guard mechanisms are additional defense in depth ([Wasmtime security](https://docs.wasmtime.dev/security.html)). WASI applications are intended to start without ambient authority and receive capabilities from the host ([WASI introduction](https://wasi.dev/)).

A conforming typst-pack deployment needs all of the following:

- Load one fixed, trusted, content-identified typst-pack guest. If the module itself is caller supplied, module decode, validation, and compilation must also be within the outer hostile boundary.
- Create a fresh Store and WASI context per attempt. Do not inherit host environment, arguments, stdio, network, clocks when semantics require explicit values, or filesystem directories. Wasmtime's `WasiCtxBuilder` defaults to no arguments, environment, preopens, or usable addresses, but TCP/UDP APIs exist; explicitly disable TCP, UDP, name lookup, and network inheritance for a no-network worker ([`WasiCtxBuilder`](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html)).
- Prefer no preopened directories. If backing files are necessary, expose only enforcement-owned immutable inputs and a private quota-backed output area with exact read/write permissions. WASI preopens are capabilities and Wasmtime prevents path traversal above the supplied directory, but the host still owns the correctness and stability of the supplied backing object.
- Configure fixed guest memory, table, instance, and stack limits. Wasmtime's `ResourceLimiter` limits guest-created memories, tables, and instances but explicitly does not account for all Store or embedder memory, so it cannot be the total process memory limit ([`ResourceLimiter`](https://docs.rs/wasmtime/latest/wasmtime/trait.ResourceLimiter.html)).
- Meter running guest code with fuel for deterministic work limits or epoch interruption for lower-overhead deadline checks. Both can trap non-cooperative Wasm execution ([Wasmtime interruption](https://docs.wasmtime.dev/examples-interrupting-wasm.html)).
- Make every host import bounded, non-blocking or independently cancellable, and charge guest-triggered host work. Wasmtime documents that fuel and epochs do not interrupt a guest blocked in a host call ([epoch interruption](https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html#method.epoch_interruption)).
- Place the entire runtime in an outer OS-confined process, cgroup/Job Object, hardened container runtime, or VM. The outer boundary limits total native memory, JIT/compiler work, file descriptors, threads, temporary storage, and host calls, and provides forced kill and reap if the runtime or a host function fails.
- Return a bounded ABI response to a separate trusted parent for verification and publication.

Without the outer boundary, server-side Wasm supports Partially Trusted work but must refuse Hostile because runtime compromise, untracked native memory, blocking host code, and process lifecycle remain in the service's trust boundary.

## Protocol Verification and Publication

The hostile worker receives no destination path, host output directory, object-store credential, publication token, terminal, or direct response stream. Its only output is a bounded protocol channel or staging object owned by the supervisor.

Before exposing any result, the parent must verify:

- exact protocol version and implementation identities;
- one expected attempt and Compilation Identity;
- bounded, canonical framing with no trailing or duplicate messages;
- exactly one terminal status and a complete report envelope;
- canonical diagnostic, dependency, and artifact counts and ordering;
- each artifact role, including output format and Source Page Number where applicable;
- each declared byte length and recomputed content identity;
- agreement between artifact identities and the terminal result; and
- absence of worker-selected physical paths or sink instructions.

Verification failure, worker crash, panic, abnormal exit, quota event, deadline, incomplete response, or confinement setup failure is a typed operational outcome. It never produces a partial Compilation Result and never retries in-process.

Publication uses private parent-owned staging and an authorized destination selected independently of worker bytes. On Linux, ordinary `rename` atomically replaces an existing destination, while `renameat2(RENAME_NOREPLACE)` fails if the destination exists when the filesystem supports it ([`rename(2)`](https://man7.org/linux/man-pages/man2/rename.2.html)). Adapters must report only the atomicity their sink actually supplies and retain the absent-destination, complete-plan, and no-partial-publication rules from [issue 35](https://github.com/sagikazarmark/typst-pack/issues/35).

## Admission and Operational Inventory

Every operation records, without secrets:

- requested Deployment Trust Profile and admission outcome;
- enforcement mechanism, version, policy identifier or digest, and accepted TCB class;
- covered filesystem, network, inherited-handle, process, IPC, device, and temporary-storage scope;
- effective transport, decoded-byte, memory, CPU, elapsed-time, process, handle, message, diagnostic, artifact, and output limits;
- immutable input handoff mechanism;
- interruption strength, complete-tree kill mechanism, and reaping result;
- worker protocol and parent-verification version;
- publication mechanism and claimed atomicity; and
- any unsupported or unverifiable property that caused refusal.

This is operational evidence, not remote attestation. A worker's self-report cannot prove its own sandbox. The trusted launcher or deployment control plane must inspect the active enforcement state.

## Verification Requirements

Hostile support should remain disabled until the target has automated deployment verification and adversarial tests. At minimum:

- Linux tests inspect actual cgroup membership and controller values, namespace and mount topology, capabilities, `no_new_privs`, seccomp and LSM activation, open descriptors, network denial, temporary quota, complete-tree kill, and empty/reaped state after termination.
- Windows tests inspect the AppContainer SID and capabilities, restricted token, Job Object membership and limits, breakaway policy, inherited handles, network denial, temporary quota, and complete-job termination.
- Container and Dagger tests inspect the actual runtime class and OCI policy rather than an image label, and prove no host mounts, sockets, credentials, service bindings, network path, privilege, or host publication capability are present.
- Service tests inject crashes, hangs, fork attempts, memory and temporary-storage exhaustion, malformed protocols, trailing messages, false hashes, duplicate artifacts, cancellation races, and worker-selected paths.
- Server-Wasm tests exhaust fuel and epochs, attempt every denied import and WASI capability, exercise host-call deadlines, exceed guest and outer memory independently, and kill a wedged runtime process.
- Browser tests may verify the Partially Trusted Worker/Wasm behavior, but the browser adapter always reports Hostile unavailable.

A probe that cannot run in the production environment is not production verification. Upgrades to the OS, runtime, container engine, Dagger engine, browser, Wasmtime, sandbox policy, worker image, or protocol invalidate prior compatibility evidence until reverified.

## Mandatory Refusals

The implementation and adapters must make these refusal rules explicit:

- `typst-pack` library calls in the caller's process refuse Hostile even if a deadline, thread, panic catcher, or cooperative cancellation token is present.
- The ordinary CLI refuses Hostile if it cannot launch the broad pre-parse boundary or if any required mechanism is unavailable.
- A CLI or service never falls back from a failed sandbox launch to the in-process Compilation Kernel.
- Ordinary Dagger and OCI execution refuses Hostile even when described as sandboxed.
- A container runtime refuses Hostile when limits are absent, soft-only, or not introspectable; when network denial is contractual only; or when host mounts, sockets, credentials, privileges, devices, or publication handles are present.
- An App Sandbox-only macOS deployment refuses Hostile.
- Browser main-thread, Dedicated Worker, and browser-Wasm execution refuse Hostile.
- Server-Wasm execution refuses Hostile without outer total-resource and process-lifecycle enforcement.
- Any target refuses Hostile when immutable handoff, complete kill/reap, bounded protocol verification, or parent-owned publication is unavailable.
- Unknown input classification is handled as Hostile or rejected. Successful parsing, a digest match, a valid Pack, or publisher authentication never permits promotion to a weaker profile.

## Architecture Recommendations

1. Introduce one target-neutral hostile-operation facility whose contract begins with immutable raw inputs and ends with a bounded untrusted terminal response. Do not overload the current Isolated Compilation Worker contract, which legitimately begins later and serves a different purpose.
2. Keep the synchronous Compilation Kernel and semantic model unchanged inside that facility. Drivers may still acquire bytes asynchronously outside, but all complex validation required for Hostile admission runs again from the same immutable bytes inside confinement.
3. Define versioned platform policy bundles for Linux, Windows, hardened OCI/Dagger, service workers, and server-side Wasm. A policy bundle states exact required mechanisms and has a fail-closed admission probe.
4. Make available profiles a runtime capability query, not a compile-time target claim. `Hostile` is absent until the active deployment verifies its policy.
5. Keep the parent protocol verifier small, bounded, format-specific, and independent of worker implementation. It must be fuzzed as part of the containment TCB.
6. Treat browser Worker/Wasm as the strongest browser Partially Trusted implementation and expose its exact interruption and resource caveats. Do not create a browser-specific weakened meaning of Hostile.
7. Keep acquisition, credentials, mutable caches, and publication outside every hostile worker. Offline policy remains orthogonal: parent network acquisition means the whole attempt is not offline even when worker network is denied.

## Residual Caveats

- Every conditional Hostile claim names and trusts an OS kernel, hypervisor, application kernel, or Wasm runtime. It is not an absolute claim against defects in that enforcement TCB.
- Hardware and microarchitectural side channels require separate deployment policy. gVisor, for example, explicitly leaves hardware side-channel defense to the host ([gVisor security model](https://gvisor.dev/docs/architecture_guide/security/)).
- CPU and deadline enforcement has platform-specific granularity and bounded overshoot. Reports must state that granularity rather than claim instantaneous termination.
- Parent verification limits output and publication effects but cannot establish semantic honesty after complete worker compromise.
- Publisher signatures and expected Pack Identity protect authenticity or replacement policy, not parser safety or confinement.
- A Hostile result does not make PDF, HTML, SVG, PNG, diagnostics, or archives safe for downstream consumers.
- Confinement does not provide confidentiality between files intentionally granted to the same compilation. Typst code authorized to read content may influence artifacts and diagnostics.
- A profile report is not remote attestation and should not be presented as one.

## Ticket and Fog Recommendation

No additional wayfinder decision ticket is required. Issue 53 can settle the target support and refusal matrix above. Concrete Rust facility seams belong to [issue 39](https://github.com/sagikazarmark/typst-pack/issues/39), CLI and Dagger presentation belongs to [issue 37](https://github.com/sagikazarmark/typst-pack/issues/37), and policy probes, malformed protocols, denial tests, crash injection, and platform verification belong to [issue 42](https://github.com/sagikazarmark/typst-pack/issues/42).

Those tickets should add implementation tasks for the broader pre-parse hostile-operation facility and versioned platform policy bundles. That is implementation decomposition of this decision, not unresolved architectural fog.

## Primary Sources

### Linux and Publication

- [Linux control group v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)
- [Linux seccomp BPF](https://docs.kernel.org/userspace-api/seccomp_filter.html)
- [Linux `no_new_privs`](https://docs.kernel.org/userspace-api/no_new_privs.html)
- [Linux Landlock](https://docs.kernel.org/userspace-api/landlock.html)
- [Linux `openat2(2)`](https://man7.org/linux/man-pages/man2/openat2.2.html)
- [Linux `memfd_create(2)`](https://man7.org/linux/man-pages/man2/memfd_create.2.html)
- [Linux `rename(2)`](https://man7.org/linux/man-pages/man2/rename.2.html)

### Containers and Dagger

- [Docker Engine security](https://docs.docker.com/engine/security/)
- [Docker resource constraints](https://docs.docker.com/engine/containers/resource_constraints/)
- [Docker seccomp profiles](https://docs.docker.com/engine/security/seccomp/)
- [Dagger sandboxed runtime](https://docs.dagger.io/features/sandbox/)
- [Dagger Container type](https://docs.dagger.io/getting-started/types/container/)
- [Dagger services](https://docs.dagger.io/features/services/)
- [Firecracker design](https://github.com/firecracker-microvm/firecracker/blob/main/docs/design.md)
- [gVisor introduction](https://gvisor.dev/docs/architecture_guide/intro/)
- [gVisor security model](https://gvisor.dev/docs/architecture_guide/security/)

### Windows and macOS

- [Windows AppContainer isolation](https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation)
- [Windows restricted tokens](https://learn.microsoft.com/en-us/windows/win32/secauthz/restricted-tokens)
- [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [Windows extended job limits](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information)
- [Windows `TerminateJobObject`](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject)
- [Windows process inheritance](https://learn.microsoft.com/en-us/windows/win32/procthread/inheritance)
- [Apple App Sandbox](https://docs.developer.apple.com/tutorials/data/documentation/security/app-sandbox.md)

### Browser, WebAssembly, and WASI

- [WHATWG HTML workers](https://html.spec.whatwg.org/multipage/workers.html)
- [W3C File API](https://w3c.github.io/FileAPI/)
- [WHATWG Storage Standard](https://storage.spec.whatwg.org/)
- [WebAssembly security](https://webassembly.org/docs/security/)
- [WebAssembly JavaScript interface](https://webassembly.github.io/spec/js-api/)
- [WASI introduction](https://wasi.dev/)
- [Wasmtime security](https://docs.wasmtime.dev/security.html)
- [Wasmtime interruption](https://docs.wasmtime.dev/examples-interrupting-wasm.html)
- [Wasmtime `ResourceLimiter`](https://docs.rs/wasmtime/latest/wasmtime/trait.ResourceLimiter.html)
- [Wasmtime epoch interruption](https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html#method.epoch_interruption)
- [Wasmtime `WasiCtxBuilder`](https://docs.rs/wasmtime-wasi/latest/wasmtime_wasi/struct.WasiCtxBuilder.html)
