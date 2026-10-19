set ignore-comments := true

# List the recipes.
[private]
default:
    @just --list --unsorted

# Run the CI jobs that run on any development machine (test-cli-parity needs x86_64 Linux).
ci: fmt clippy doc audit build test test-features build-wasm check-windows

# Build the workspace.
build:
    cargo build --workspace --all-features --locked

# Check formatting.
fmt:
    cargo fmt --all --check

# Run clippy.
clippy:
    cargo clippy --workspace --all-targets --all-features --locked -- --deny warnings

# Run the complete test suite with every crate feature enabled.
test:
    cargo test --workspace --all-features --locked

# Run the suites whose feature gates the all-features build would hide.
test-features: test-core test-fs test-opendal test-cli

# Run the featureless core suite, including identity compatibility checks.
test-core:
    cargo test --package typst-pack --no-default-features --locked

# Run the filesystem suite for a build with no HTTP client in its dependency graph.
test-fs:
    #!/usr/bin/env bash
    set -euo pipefail

    # Every crate a typst-pack build links, dev-dependencies excluded, as one
    # bracketed `name version` entry each, so a name matches a crate rather
    # than part of one.
    graph() {
        cargo tree --locked --package typst-pack --no-default-features --features "$1" \
            --edges normal,build --prefix none --format '[{p}]'
    }
    without_egress=$(graph fs)
    with_egress=$(graph egress)

    # Every crate this manifest can activate that speaks HTTP. The egress build
    # links each of them, so checking it too keeps a name that matches nothing
    # from passing as an absent transport.
    for transport in env_proxy ureq webpki-roots; do
        if grep -qF "[$transport v" <<<"$without_egress"; then
            echo "the fs feature links the HTTP transport crate $transport" >&2
            exit 1
        fi
        if ! grep -qF "[$transport v" <<<"$with_egress"; then
            echo "the egress feature no longer links $transport; update this list" >&2
            exit 1
        fi
    done

    cargo test --package typst-pack --no-default-features --features fs --locked

# Run the OpenDAL suites with only their own features, audit the packaged feature graph, and build its docs.
test-opendal:
    cargo test --package typst-pack --no-default-features --features opendal --locked --lib opendal::tests
    cargo test --package typst-pack --no-default-features --features opendal --locked --test opendal_adapter
    cargo test --package typst-pack --no-default-features --features opendal,package-reading --locked --test opendal_adapter
    bash scripts/check-opendal-compatibility.sh
    cargo doc --package typst-pack --no-default-features --features opendal --no-deps --locked

# Run the ordinary CLI test suite without internal test-only features.
test-cli:
    cargo test --package typst-pack-cli --test cli --locked

official_typst := justfile_directory() / "target" / "official-typst"

# Gate shared CLI behavior against the exact embedded Typst release (x86_64 Linux only).
test-cli-parity:
    bash scripts/install-official-typst.sh crates/typst-pack/embedded-typst.toml "{{ official_typst }}"
    cargo build --release --package typst-pack-cli --bin typst-pack --locked
    TYPST_PACK_OFFICIAL_TYPST="{{ official_typst }}/typst" \
    TYPST_PACK_REQUIRE_OFFICIAL_TYPST=1 \
    TYPST_PACK_TEST_BINARY="{{ justfile_directory() }}/target/release/typst-pack" \
        cargo test --package typst-pack-cli --all-features --test official_typst_cli --locked

# Keep the featureless core, Pack Creation and fixed Document Time included, building on wasm.
build-wasm:
    cargo build --package typst-pack --no-default-features --target wasm32-unknown-unknown --locked

# Type-check the cfg(windows) code paths without a Windows runner.
check-windows:
    cargo check --package typst-pack --no-default-features --features fs,opendal,package-reading,embedded-fonts --target x86_64-pc-windows-gnu --locked

# Run the platform-dependent suites on the host; run it on Linux, macOS, and Windows.
test-platform:
    # Compile coverage only: the OpenDAL adapter has no platform-specific code.
    cargo check --package typst-pack --no-default-features --features opendal --locked
    cargo check --package typst-pack --no-default-features --features opendal,package-reading --locked
    # The filesystem adapter is where platform behavior lives: symlink and
    # alias refusal, entry kinds, staging, and commit all differ per target.
    cargo test --package typst-pack --no-default-features --features fs,opendal,package-reading,embedded-fonts --locked --test creation_conformance
    cargo test --package typst-pack --features fs,embedded-fonts --locked --lib fs::
    cargo test --package typst-pack --features fs,embedded-fonts --locked --test fs --test pack
    # Writing, paths, and stdout are also platform behavior the CLI exposes.
    cargo test --package typst-pack-cli --locked --test cli --test lifecycle_compatibility

# Build the documentation, denying rustdoc warnings.
[env("RUSTDOCFLAGS", "--deny warnings")]
doc:
    cargo doc --workspace --all-features --no-deps --locked

# Audit Cargo.lock for vulnerable crates.
audit:
    cargo audit
