# Project Override and Layering Precedents

Research date: 2026-07-18

## Question and method

How do first-party Typst systems and comparable portable-project, archive,
build, and virtual-filesystem systems model explicit compilation-scoped file
replacement, including source and entrypoint replacement, package and font
authority, ordering, declaration, trust, diagnostics, provenance, and
materialization?

This survey uses only primary sources: first-party documentation and source
code, and authoritative specifications. Typst compiler, CLI, package, and
template source findings are pinned to Typst 0.15.0, the version embedded by the
repository at the time of research; web-app behavior comes from the current
first-party documentation. The comparisons are Go 1.26.5, LLVM/Clang 21.1.8,
Bazel 9.0.0, and OCI Image Specification 1.1.1.

## Executive findings

- First-party Typst has no CLI or web-app concept that means "replace this
  project file for one compilation." The compiler can nevertheless express
  both entrypoint selection and arbitrary file substitution through the
  caller-owned `World`: `main()` selects a `FileId`, while `source()` and
  `file()` supply its contents. This is an integration seam, not a declared
  override product model.
- Typst project files, package files, and fonts are separate authority domains.
  A `FileId` distinguishes project and package roots; the first-party CLI routes
  package-rooted requests through one package root selected in data, cache,
  download order. Fonts do not pass through file lookup at all: the `World`
  supplies an ordered `FontBook` and indexed font values.
- Go's `-overlay` is the closest direct precedent for explicit,
  invocation-scoped replacement. One JSON `Replace` map redirects exact logical
  disk paths to backing paths, can make missing paths appear, and can hide paths
  with an empty backing value. It preserves the logical name while retaining
  internal access to the actual path. It deliberately excludes the module
  cache.
- Clang's VFS overlays are the richest layering precedent. YAML maps virtual
  files or directories to external contents; repeated overlay files have a
  specified bottom-to-top order; lookup can be redirect-first, base-first, or
  redirect-only; and diagnostic naming can expose either the virtual or
  external path. It is not a confinement mechanism.
- Bazel separates replacement from injection. `--override_repository` may
  replace a repository already visible by name, while `--inject_repository`
  adds one; `--override_module` is required when replacement must also control
  module metadata. This is a useful authority and validation precedent, but its
  unit is a whole repository or module rather than a file or entrypoint.
- OCI layers are a strong archive/materialization precedent, not a
  compilation-scoped input precedent. Layers are ordered changesets; a later
  ordinary entry replaces an existing non-directory path by remove-and-recreate,
  and whiteouts model deletion. The effective tree, layer-chain identity, and
  runtime entrypoint are separate concerns.
- Across these systems, `overlay` denotes a composed virtual view, `override`
  denotes a higher-authority substitution, `replacement` denotes the direct
  old-to-new mapping, and `patch` denotes a transformation applied to a known
  base. These terms overlap, but they do not imply the same eligibility,
  persistence, or materialization semantics.

## First-party Typst behavior

### Compiler model: identity and bytes are supplied by the world

The compiler's `World` interface separately asks for the main `FileId`, source
text by `FileId`, raw bytes by `FileId`, font metadata, and a font by index
([`World`, lines 60-88](https://github.com/typst/typst/blob/v0.15.0/crates/typst-library/src/lib.rs#L60-L88)).
Compilation obtains `world.main()` and then fetches that same identity through
`world.source(main)` before evaluation
([compiler entry fetch, lines 116-126](https://github.com/typst/typst/blob/v0.15.0/crates/typst/src/lib.rs#L116-L126)).

Consequently, an embedding can model two distinct operations without changing
the compiler:

- **Replace the selected entrypoint's bytes:** keep `main()` unchanged and
  return different source text for that `FileId`.
- **Replace entrypoint selection:** make `main()` return another `FileId`, then
  serve that file normally.

The same `source()` and `file()` methods serve imports, includes, images, data,
and other project reads. A custom world can therefore return replacement bytes
for any requested identity, including Typst source. Typst calls this custom file
loading, not an override. `typst-kit` explicitly says clients can implement
their own `FileLoader`, or bypass `FileStore` and implement `World::source` and
`World::file` directly
([`FileLoader`, lines 248-265](https://github.com/typst/typst/blob/v0.15.0/crates/typst-kit/src/files.rs#L248-L265)).

This seam does not itself provide a replacement manifest, target eligibility,
multiple-layer order, base-content precondition, or provenance record. Those
are responsibilities of the `World` implementation.

### Project and package authority are distinct

A Typst path identity consists of a virtual path in either the project root or
a package root identified by a complete package specification
([`VirtualRoot`, lines 74-98](https://github.com/typst/typst/blob/v0.15.0/crates/typst-syntax/src/path.rs#L74-L98)).
The standard file loader explicitly switches on that root: project requests use
the project filesystem root, while package requests obtain the package root
for the requested package specification
([standard loader, lines 303-318](https://github.com/typst/typst/blob/v0.15.0/crates/typst-kit/src/files.rs#L303-L318)).
Thus, `logo.svg` in a project and `logo.svg` in a package are not competing
layers; they are different identities.

Package import first reads `typst.toml` inside the package root, validates the
manifest against the requested package name and version, and resolves the
manifest's package entrypoint in that same package root
([package resolution, lines 258-283](https://github.com/typst/typst/blob/v0.15.0/crates/typst-eval/src/import.rs#L258-L283)).
The first-party CLI selects one root for the whole requested package in this
order: local data directory, package cache, then a downloaded Typst Universe
archive stored in the cache
([package lookup, lines 95-123](https://github.com/typst/typst/blob/v0.15.0/crates/typst-kit/src/packages.rs#L95-L123)).
This is first-success authority at package-root granularity, not per-file
fallback across package providers.

A custom `World` can intercept package-rooted `FileId`s because the compiler
delegates them too, but the first-party CLI offers no package-file overlay. Its
`--package-path` and `--package-cache-path` each replace the corresponding
storage root
([package flags, lines 452-463](https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/args.rs#L452-L463));
they do not merge project files into packages.

### Fonts are not files in the project lookup domain

Fonts are selected through `World::book()` and `World::font(index)`, not
`World::file()`
([`World`, lines 66-88](https://github.com/typst/typst/blob/v0.15.0/crates/typst-library/src/lib.rs#L66-L88)).
The 0.15.0 CLI adds font providers in this order: system fonts, Typst embedded
fonts, then each `--font-path` directory in argument order
([CLI font discovery, lines 38-54](https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/fonts.rs#L38-L54)).
For equal matching scores, `FontBook` retains the earlier font because it only
replaces the current best when the new score is strictly greater
([font matching, lines 139-163](https://github.com/typst/typst/blob/v0.15.0/crates/typst-library/src/text/font/book.rs#L139-L163)).

The transferable fact is the authority boundary: replacing a project path does
not replace a font, even if the replacement bytes happen to contain a font.
Font precedence must be modeled independently from file precedence.

### CLI and web-app behavior do not add an override layer

The CLI exposes a project root, string-valued `sys.inputs`, fonts, packages,
and creation time, but no file-overlay or replacement option
([`WorldArgs`, lines 399-429](https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/args.rs#L399-L429)).
The input path becomes the project-rooted main `FileId`; all other project and
package reads route directly to their selected roots
([CLI main and root routing, lines 227-268](https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/world.rs#L227-L268)).
Standard Typst path semantics confine project paths to the project root and
package code to its own package root; in the web app, the project itself is the
root and the preview toggle merely chooses which Typst file is compiled
([Typst path roots](https://typst.app/docs/reference/foundations/path/#roots)).

Templates are materialization, not compilation layering. A template manifest
declares a directory to copy and an entrypoint relative to that directory
([`TemplateInfo`, lines 80-90](https://github.com/typst/typst/blob/v0.15.0/crates/typst-syntax/src/package.rs#L80-L90)).
`typst init` refuses an existing destination and copies the template directory's
contents into a new project directory
([scaffolding, lines 69-92](https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/init.rs#L69-L92)).
The web app likewise documents that a private template subdirectory is copied
as-is and that files outside it are not copied
([private templates](https://typst.app/docs/web-app/private-packages/#private-templates)).
After copying, the new project owns ordinary files; no live template layer
remains.

### Security, diagnostics, and provenance

Typst's virtual paths reject lexical escape from a root. The standard
filesystem adapter notes that its path join can still escape through symlinks
([`FsRoot::load`, lines 342-358](https://github.com/typst/typst/blob/v0.15.0/crates/typst-kit/src/files.rs#L342-L358)).
An override implementation that reads arbitrary backing paths would therefore
create a separate trust boundary; Typst's logical project-root check does not
authenticate or confine such a provider.

Compiler spans carry the logical `FileId`. The CLI renders project diagnostics
as project paths and package diagnostics as `@namespace/name:version/path`
([diagnostic naming, lines 147-164](https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/world.rs#L147-L164)).
The CLI dependency API instead resolves accessed identities to backing
filesystem paths when possible
([dependencies, lines 98-101](https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/world.rs#L98-L101)).
This demonstrates two useful but separate views: stable logical identity for
source diagnostics, and backing-location provenance for host dependency
tracking. First-party Typst does not define how an overlaid file should appear
in either view.

Private package access in the web app is authorization, not lookup precedence:
external project collaborators receive configurable, read-only access to
private packages used by the project
([package access](https://typst.app/docs/web-app/private-packages/#private-packages-and-external-collaborators)).
That is evidence that package trust is independent from project file authority.

## Go build overlays

### Model and scope

Go's `-overlay` flag reads one JSON object whose `Replace` map maps a disk path
to a backing path. The build behaves as if the logical path had the backing
file's contents; an empty backing path makes the logical path absent
([build flag, lines 169-180](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/work/build.go#L169-L180)).
The implementation describes its purpose as compiling editor buffers that have
not yet been saved to their final locations
([`fsys`, lines 5-10](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/fsys/fsys.go#L5-L10)).
This is invocation-scoped and leaves both the logical project and backing files
unchanged.

The map is explicit at file-path granularity. It can replace an existing file,
inject a file at a path absent on disk, or delete a path for the virtual build.
Directory listings merge ordinary and overlaid children, so injected source
files participate in package discovery. There is one public overlay map, not an
ordered list of public layers. Keys and values are normalized to absolute
paths; duplicate normalized keys and file/child structural conflicts are
rejected
([overlay validation, lines 361-401](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/fsys/fsys.go#L361-L401)).
For paths not named by the map, disk is the base layer.

### Source, entrypoint, and package authority

The overlay covers build source trees. `go build` can take a list of `.go`
files as the source set for a package
([build inputs, lines 36-50](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/work/build.go#L36-L50)),
so a selected source path can be redirected through the overlay. Go has no
single source-file entrypoint analogous to Typst's main file: executable entry
is the `main` package and its `main` function. The precedent therefore supports
source-set replacement, but not a separate persisted entrypoint field.

Files under `GOMODCACHE` may not be replaced
([overlay limitations, lines 175-180](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/work/build.go#L175-L180)).
The implementation exposes `DirContainsReplacement` specifically so callers can
reject overlays affecting that immutable authority domain
([module-cache guard, lines 523-546](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/fsys/fsys.go#L523-L546)).
Whole dependency replacement is instead a module-graph operation: a main
module's `replace` directive substitutes a required module version with another
module or local module root, and is ignored when that module is merely a
dependency
([Go `replace` directive](https://go.dev/ref/mod#go-mod-file-replace)).

This separation is directly relevant: exact project-file replacement and
package authority use different mechanisms and eligibility rules.

### Trust, diagnostics, provenance, and materialization

The overlay names arbitrary local backing paths. Validation catches malformed
or structurally inconsistent maps, but the format has no digest, signature,
allowlist of original files, or backing-path sandbox. Trust comes from the
caller selecting both the JSON file and its backing files.

The virtual filesystem opens the actual backing file while preserving the
logical name in synthesized file metadata. It also exposes `Actual()` and
`Replaced()` internally
([logical and actual paths, lines 503-520](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/fsys/fsys.go#L503-L520))
and can trace requested operations against logical paths with `gofsystrace`
([trace, lines 34-50](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/fsys/fsys.go#L34-L50)).
This is a useful dual-provenance pattern even though the public build output
does not emit an override report.

There is no extraction or merged-tree output. The overlay is a read-time build
view. The documentation also warns that overlay files do not appear when
programs and tests are run through `go run` and `go test`; only the build uses
the virtual contents
([limitations, lines 175-180](https://github.com/golang/go/blob/go1.26.5/src/cmd/go/internal/work/build.go#L175-L180)).

### Fit

Transferable: exact logical-to-backing mappings, virtual injection/deletion,
strict structural validation, base fallback, compilation scope, preserved
logical diagnostics, backing-path introspection, and an explicit package-cache
exclusion.

Mismatched: host-absolute identities rather than portable project-relative
ones; no multiple-layer order; deletion and injection are broader than
replacement; no persisted entrypoint; local backing paths are inherently
filesystem-bound; and run behavior is deliberately not the same as build
behavior.

## LLVM and Clang virtual filesystem overlays

### Model and layer order

Clang exposes `-ivfsoverlay` and `-vfsoverlay` to place a described virtual
filesystem over the real filesystem
([option definitions, lines 4821-4827](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/clang/include/clang/Driver/Options.td#L4821-L4827)).
The YAML `RedirectingFileSystem` can declare individual virtual files,
explicit virtual directories, or whole directory remaps to external files or
directories
([YAML entries, lines 722-765](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/llvm/include/llvm/Support/VirtualFileSystem.h#L722-L765)).

Repeated overlay files have a precise command-line order: earlier VFS files are
on the bottom; each later file wraps the accumulated filesystem and therefore
has higher authority
([Clang construction, lines 5450-5476](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/clang/lib/Frontend/CompilerInvocation.cpp#L5450-L5476)).
At the generic VFS level, directories merge as a union, topmost directory
attributes win, and a topmost file overrides lower files
([`OverlayFileSystem`, lines 384-406](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/llvm/include/llvm/Support/VirtualFileSystem.h#L384-L406)).

Within a redirecting overlay, `redirecting-with` selects one of three policies:
redirected path first then original (`fallthrough`), original first then
redirected (`fallback`), or redirected path only (`redirect-only`)
([configuration defaults, lines 704-712](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/llvm/include/llvm/Support/VirtualFileSystem.h#L704-L712);
[lookup modes, lines 789-800](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/llvm/include/llvm/Support/VirtualFileSystem.h#L789-L800)).
This makes both layer order and miss behavior explicit rather than treating all
errors as fallthrough.

### Source, entrypoint, declaration, and authority

The overlay applies to Clang's compilation filesystem, not merely header search,
so file mappings can cover source paths as well as includes. There is no
separate overlay entrypoint field: the command line still selects the main
input path, and mapping that virtual path redirects the selected source's
contents. Directories permit broad remapping; individual `file` entries provide
an exact declaration surface.

The YAML roots are a declaration of virtual coverage, but not an allowlist
against the base filesystem under the default `fallthrough` mode. A
`redirect-only` overlay can make the declared virtual tree closed for matching
redirects, while multiple VFS layers and the external filesystem remain
separate composition choices. Clang has no built-in concept of package files or
fonts in this mechanism; include directories, module maps, SDKs, and source
files are all paths in the compilation VFS.

### Trust, diagnostics, provenance, and materialization

Roots can be relative to the working directory or overlay file, and external
contents can point to external paths. `overlay-relative` changes path
interpretation but does not confine access
([path configuration, lines 699-720](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/llvm/include/llvm/Support/VirtualFileSystem.h#L699-L720)).
The VFS format is therefore a redirection mechanism, not a sandbox or trust
format. It contains no content hashes or signatures.

Provenance is explicit and configurable. Global or per-entry
`use-external-name` controls whether status reports the external remapped name
or the virtual name used for access, specifically to support communication with
users and tools outside the VFS
([external naming, lines 755-780](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/llvm/include/llvm/Support/VirtualFileSystem.h#L755-L780)).
Clang's module dependency collector also records every virtual-to-real mapping
from every overlay
([mapping collection, lines 261-279](https://github.com/llvm/llvm-project/blob/llvmorg-21.1.8/clang/lib/Frontend/CompilerInstance.cpp#L261-L279)).

No merged project is extracted. The mappings are consumed as a virtual view;
dependency and reproducer machinery can copy or report mapped files as a
separate concern.

### Fit

Transferable: explicit virtual and backing identities, file versus directory
declarations, multiple ordered layers, precisely named miss policies, virtual
versus external diagnostic names, and complete mapping collection for
provenance.

Mismatched: arbitrary external paths and directory remaps are broader than
contained-project replacement; default fallthrough exposes undeclared base
files; there is no package/font authority boundary; and the overlay does not
define archive extraction or a portable backing-byte representation.

## Bazel repository and module overrides

### Model, declaration, and authority

Bazel's command-scoped `--override_repository=name=path` replaces a visible
repository with a local directory. `--inject_repository=name=path` is a
different operation that adds a repository, and `--override_module=name=path`
replaces module resolution from a local directory
([repository options, lines 202-262](https://github.com/bazelbuild/bazel/blob/9.0.0/src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryOptions.java#L202-L262)).
The distinction is enforced: overriding an apparent repository name that is
not visible is an error suggesting `--inject_repository`
([visibility diagnostic, lines 212-230](https://github.com/bazelbuild/bazel/blob/9.0.0/src/main/java/com/google/devtools/build/lib/bazel/repository/RepoDefinitionFunction.java#L212-L230)).

This is declaration by logical authority name, not path. A repository override
selects one replacement root for all files and build metadata in that
repository. The separate module flag matters because a repository override of a
registry module does not make replacement `MODULE.bazel` changes effective
([flag warning, lines 211-217](https://github.com/bazelbuild/bazel/blob/9.0.0/src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryOptions.java#L211-L217)).

Persistent `MODULE.bazel` overrides are root-authority decisions: only the root
module's overrides apply, and they may target transitive dependencies. Bazel
distinguishes version/registry/patch overrides from archive, Git, and local-path
overrides that remove a module from registry version resolution
([module overrides, lines 116-182](https://github.com/bazelbuild/bazel/blob/9.0.0/site/en/external/module.md#L116-L182)).
This demonstrates that a `patch` is a transformation of fetched package
content, while a local-path `override` changes the authoritative provider for
the whole module.

### Ordering and diagnostics

Bazel does not stack arbitrary repository roots. Its resolution code checks
command-line overrides by canonical name first, then non-registry module
overrides, general apparent-name overrides, resolved module repositories, and
module-extension repositories
([repository resolution, lines 95-166](https://github.com/bazelbuild/bazel/blob/9.0.0/src/main/java/com/google/devtools/build/lib/bazel/repository/RepoDefinitionFunction.java#L95-L166)).
Within the non-registry stage, an apparent-name command-line override for a
direct dependency takes precedence over that non-registry override
([direct-dependency exception, lines 181-190](https://github.com/bazelbuild/bazel/blob/9.0.0/src/main/java/com/google/devtools/build/lib/bazel/repository/RepoDefinitionFunction.java#L181-L190)).
Within a named repository there is one authority, so there is no per-file
fallback chain.

Diagnostics identify apparent or canonical repository names and fail on unknown
authority rather than silently treating an override as an injection. Repository
rules also have declared attribute schemas, and a reproducible rule can return
the fixed parameters that reproduce a fetched repository, such as a commit in
place of a floating branch
([repository rule contract, lines 19-75](https://github.com/bazelbuild/bazel/blob/9.0.0/site/en/external/repo.md#L19-L75)).
The Bzlmod lockfile records resolution results and hashes of remote registry
inputs
([Bazel lockfile](https://github.com/bazelbuild/bazel/blob/9.0.0/site/en/external/lockfile.md#lockfile-contents)),
but a command-line local path remains caller-authorized mutable input rather
than signed content.

### Source, entrypoint, packages, and materialization

Repository overrides can replace any source or build file inside an external
repository as a consequence of replacing the entire root. They do not replace
individual files in the main repository, and Bazel targets rather than one
source file serve as build entrypoints. Bazel's repository/module unit is a
strong package-authority precedent but a poor direct source-file precedent.

The replacement repository is read from its local root and may be represented
inside Bazel's external repository machinery, but the command is not an archive
extraction contract and does not produce a merged source tree for the user.

### Fit

Transferable: replacement versus injection as distinct operations; override
eligibility by declared/visible logical name; package metadata authority
separate from already-resolved repository contents; explicit precedence among
authority sources; failure on unknown names; and lockfile provenance for remote
resolution.

Mismatched: whole repositories/modules rather than files; no project entrypoint;
no per-path layer order; local filesystem roots rather than portable bytes; and
patches depend on package-resolution and fetched-base semantics.

## OCI image filesystem layers

### Ordered replacement and deletion

An OCI image manifest declares the base layer at index zero and subsequent
layers in stack order; applying them to an empty directory must produce the
final filesystem
([manifest layer order, lines 70-73](https://github.com/opencontainers/image-spec/blob/v1.1.1/manifest.md#L70-L73)).
Layers represent additions, modifications, and removals. Additions and
modifications carry full entries; removals use whiteouts.

Applying a layer is intentionally not ordinary archive extraction. If an entry
collides with an existing non-directory path, the implementation removes the
old path and recreates it from the new entry. Whiteouts delete only lower-layer
resources and are hidden after application
([application and whiteouts, lines 229-252](https://github.com/opencontainers/image-spec/blob/v1.1.1/layer.md#L229-L252)).
Opaque whiteouts can hide all lower children of a directory
([opaque whiteouts, lines 276-302](https://github.com/opencontainers/image-spec/blob/v1.1.1/layer.md#L276-L302)).

This is a complete ordered changeset model, including addition and deletion,
not an allowlist of replacements for paths known to the base. Layer order is
persistent in a derived image rather than supplied for one compilation.

### Entrypoint, trust, provenance, and materialization

Filesystem content and entrypoint metadata are separate. The image
configuration's `Entrypoint` is a runtime default that container creation may
replace independently of filesystem layers
([entrypoint, lines 157-166](https://github.com/opencontainers/image-spec/blob/v1.1.1/config.md#L157-L166)).
This is useful evidence for distinguishing "replace the bytes at the existing
entrypoint path" from "select a different entrypoint," even though the OCI
entrypoint is an executable argument vector rather than a source identity.

Each layer descriptor carries media type, digest, and size. Consumers of
untrusted content should verify size and digest before heavy processing
([descriptor verification, lines 105-110](https://github.com/opencontainers/image-spec/blob/v1.1.1/descriptor.md#L105-L110)).
`ChainID` hashes the ordered application of layer `DiffID`s, distinguishing a
changeset from that changeset applied to a particular chain
([chain identity, lines 33-58](https://github.com/opencontainers/image-spec/blob/v1.1.1/config.md#L33-L58)).
Optional ordered history can carry author, creation time, command, and comment
for each layer
([history, lines 224-249](https://github.com/opencontainers/image-spec/blob/v1.1.1/config.md#L224-L249)).
These provide integrity and provenance, not publisher authorization; the image
specification does not define signature trust policy.

OCI explicitly materializes the effective tree by applying changesets in order
when producing a runtime bundle
([image layout, lines 1-10](https://github.com/opencontainers/image-spec/blob/v1.1.1/image-layout.md#L1-L10)).
Whiteout marker files do not survive into that tree. The layer specification
defines replacement and whiteout semantics but does not define a root-confining
archive extraction policy; it is therefore not evidence that arbitrary tar
paths or links are safe to materialize.

### Fit

Transferable: canonical bottom-to-top order; whole-entry replacement semantics;
explicit deletion markers that disappear after application; separate
entrypoint metadata; content digests; an order-sensitive chain identity;
optional per-layer history; and a clear distinction between applying a
changeset and extracting an archive.

Mismatched: persistent derived filesystem rather than compilation scope;
addition and deletion of arbitrary paths; no base-path allowlist; operating
system metadata and special file types; runtime executable entrypoints; no
package/font domains; and no specified archive path sandbox.

## Cross-system comparison

| Concern | Direct precedents | Important limit |
| --- | --- | --- |
| Source replacement | Typst custom `World`; Go exact file map; Clang file remap | Typst has no first-party declared override; Go and Clang can also inject paths |
| Entrypoint bytes | Typst keeps `main()` identity and changes `source(main)`; Clang/Go can remap the selected source path | Go has no single source entrypoint; Clang has no persisted entrypoint field |
| Entrypoint selection | Typst changes `World::main()`; OCI replaces entrypoint metadata independently | OCI is runtime metadata, not source compilation |
| Package authority | Typst package-root identity and data/cache/download order; Go module `replace`; Bazel repository/module override | Package systems replace whole roots or graph nodes, not arbitrary project files |
| Font authority | Typst's separate ordered `FontBook` | File overlays do not model font matching or authority |
| Layer ordering | Clang later overlay files are higher; OCI applies manifest order; Bazel has authority-resolution precedence | Go exposes one unordered replacement map; Typst defines no override order |
| Declaration and allowlisting | Go exact keys; Clang explicit file roots; Bazel visible logical names | Clang directory remaps and default fallthrough are broad; OCI layers have no base allowlist |
| Missing and errors | Clang names redirect-first/base-first/redirect-only; Go falls through only for unlisted paths and explicitly maps deletion | No surveyed system establishes a universal "all errors fall through" rule |
| Trust and security | Typst project/package roots; Go protects immutable module cache; Bazel validates visible authority names; OCI verifies content digest and size | Local backing paths in Go, Clang, and Bazel are caller-trusted; digest integrity is not signer authorization |
| Diagnostics and provenance | Typst logical `FileId` plus backing dependencies; Go logical names plus `Actual`; Clang selectable virtual/external names and mapping collection; OCI descriptors/ChainID/history | Provenance is optional or internal in several systems; none defines Typst override diagnostics |
| Extraction/materialization | Typst templates copy once; OCI applies changesets; Go and Clang stay virtual | Template scaffolding is not layering; OCI whiteouts and special files are a much broader archive model |

## Transferable patterns

These are patterns supported by the precedents, not decisions for typst-pack.

1. **Keep logical identity separate from backing bytes.** Typst `FileId`, Go's
   overlaid path, and Clang's virtual path remain stable while another source
   supplies bytes. This preserves relative resolution and source spans.
2. **Treat entrypoint selection and entrypoint content as different changes.**
   Typst's `main()` versus `source(main)` and OCI's entrypoint metadata versus
   layers make the distinction explicit.
3. **Separate authority domains before defining order.** Typst project roots,
   package roots, and font books are not interchangeable layers. Go and Bazel
   likewise keep package/module replacement separate from project file
   overlays.
4. **Make replacement and injection observably different when eligibility
   matters.** Bazel rejects override of an unknown visible repository and names
   injection separately. Go and Clang do not make this distinction and thus
   demonstrate the broader semantics that result.
5. **Specify order and miss behavior independently.** Clang separately defines
   layer order and redirect/base lookup policy. A sequence alone does not say
   whether only absence, or any error, advances lookup.
6. **Reject ambiguous normalized declarations.** Go canonicalizes paths and
   rejects duplicate or structurally inconsistent entries before the build.
7. **Expose both logical and backing provenance.** Typst diagnostics versus
   dependencies, Go virtual versus actual paths, and Clang's naming and mapping
   collection show why one path is insufficient for both user-facing errors and
   audit/dependency reporting.
8. **State whether the result is virtual or materialized.** Go and Clang never
   merge files on disk; templates copy once; OCI applies a persistent
   changeset. Extraction semantics should not be inferred from compilation
   lookup semantics.
9. **Separate integrity, authorization, and confinement.** OCI digests prove
   bytes match an expected identity; Typst roots constrain logical access;
   private-package ACLs authorize readers. None substitutes for the others.
10. **If layers are content-addressed, bind identity to order.** OCI's ChainID
    demonstrates that hashing an individual replacement layer does not identify
    the effective tree without its ordered base chain.

## Mismatched precedents

- **Go and Clang overlays are broader than replacement.** Both can make a file
  appear where the base has none; Go can delete paths, and Clang can remap whole
  directories or expose undeclared base paths. Their syntax should not be read
  as evidence for replacement eligibility.
- **Go module `replace` and Bazel overrides are package-graph authority.** They
  preserve dependency names while changing whole providers. They do not answer
  whether a contained project file, source file, or entrypoint may be replaced.
- **Bazel patches are transformations.** A patch describes edits relative to a
  fetched base and may fail to apply; a replacement provides complete bytes for
  one logical identity. The terms should not be treated as synonyms.
- **OCI layers are durable derivation.** Their arbitrary additions, deletions,
  metadata, whiteouts, and special files solve runtime filesystem construction,
  not a contained compilation view.
- **OCI entrypoint override is not source replacement.** It changes the runtime
  command and can name a different executable; it does not preserve a source
  file identity or Typst-relative import base.
- **Typst templates are copy-on-create.** The template entrypoint guides the new
  project, but the copied files become ordinary project content before
  compilation. There is no fallback to or provenance link with the template.
- **Font precedence is not path precedence.** Typst scores font metadata and
  resolves ties by insertion order; replacing a file path cannot reproduce
  those semantics.
- **None of the surveyed formats supplies typst-pack policy.** In particular,
  they do not decide whether replacement targets must already be contained,
  whether the entrypoint identity may change, whether multiple providers are
  useful, whether package/font targets should be eligible, or what extraction
  should emit.

## Terminology

| Term | Primary-source usage | Semantic signal |
| --- | --- | --- |
| **Override** | Bazel repository/module overrides and module-resolution overrides | A higher-authority choice supersedes the normal provider or resolver result; granularity must be stated |
| **Overlay** | Go's JSON build overlay; Clang/LLVM virtual filesystem overlays | A virtual view composes mappings with a base; may imply fallback, injection, deletion, or directories unless narrowed |
| **Replacement / replace** | Go overlay's `Replace` map; Go module `replace`; OCI remove-and-recreate collision behavior | Direct substitution of contents or provider while retaining a logical target; used at both file and module granularity |
| **Patch** | Bazel `single_version_override` patch attributes | A delta transformed against known fetched content, not complete replacement bytes |
| **Layer / changeset** | OCI ordered filesystem layers | A persistent ordered delta that may add, modify, and remove paths and whose meaning depends on its base chain |
| **Redirect** | LLVM `RedirectingFileSystem` | Resolve one virtual name through another backing location, with an explicit lookup policy |
| **Injection** | Bazel `--inject_repository` | Introduce a previously unavailable logical authority, deliberately distinguished from override |
| **Whiteout** | OCI layer deletion marker | Hide a lower-layer path during materialization; not a missing-value fallback |

The precedents support using these words only with an explicit unit and
lifecycle, such as "file replacement for one compilation" or "ordered virtual
filesystem overlay." The terminology alone does not determine typst-pack's
policy.
