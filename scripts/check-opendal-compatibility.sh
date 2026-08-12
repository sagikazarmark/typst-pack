#!/usr/bin/env bash

set -euo pipefail

workspace=$(pwd)
audit=$(mktemp -d)
trap 'rm -rf "$audit"' EXIT

cargo package \
  --package typst-pack \
  --allow-dirty \
  --no-verify \
  --target-dir "$audit/package-target"

packages=("$audit"/package-target/package/typst-pack-*.crate)
if [[ ${#packages[@]} -ne 1 || ! -f ${packages[0]} ]]; then
  printf 'expected one packaged typst-pack crate, found %s\n' "${#packages[@]}" >&2
  exit 1
fi

mkdir "$audit/packaged"
tar -xzf "${packages[0]}" -C "$audit/packaged"
packaged=("$audit"/packaged/typst-pack-*)
if [[ ${#packaged[@]} -ne 1 || ! -d ${packaged[0]} ]]; then
  printf 'expected one unpacked typst-pack directory, found %s\n' "${#packaged[@]}" >&2
  exit 1
fi

mkdir "$audit/enabled" "$audit/enabled/src"
cat >"$audit/enabled/Cargo.toml" <<EOF
[package]
name = "opendal-enabled-consumer"
version = "0.0.0"
edition = "2024"
rust-version = "1.92"

[dependencies]
opendal = { version = "0.58", default-features = false }
typst-pack = { path = "${packaged[0]}", default-features = false, features = ["opendal"] }
EOF
cat >"$audit/enabled/src/main.rs" <<'EOF'
use typst_pack::opendal::{OperatorBinding, OperatorBindings, OperatorResolver};

fn resolve_for_consumer<R: OperatorResolver>(
    resolver: &R,
    binding: &OperatorBinding,
) -> Result<opendal::Operator, R::Error> {
    resolver.resolve(binding)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let operator = opendal::Operator::new(opendal::services::Memory::default())?;
    let archive = OperatorBinding::new("archive")?;
    let project = OperatorBinding::new("project")?;
    let bindings = OperatorBindings::new([
        (project, operator.clone()),
        (archive.clone(), operator),
    ])?;

    assert_eq!(
        bindings
            .bindings()
            .map(OperatorBinding::as_str)
            .collect::<Vec<_>>(),
        ["archive", "project"]
    );
    let _direct_operator = bindings.operator(&archive).expect("archive is configured");
    let _resolved_operator = resolve_for_consumer(&bindings, &archive)?;

    Ok(())
}
EOF

# This lockfile is created outside the workspace, so resolution cannot inherit
# the workspace lock. Behavioral tests use Cargo's dev/test feature-unified
# superset; this consumer is the isolated normal/build graph evidence.
cargo check --manifest-path "$audit/enabled/Cargo.toml"
cargo tree \
  --manifest-path "$audit/enabled/Cargo.toml" \
  --edges normal,build \
  --prefix none \
  --format '[{p}] features=[{f}]' \
  >"$audit/enabled/normal-build-tree.txt"
cargo metadata \
  --manifest-path "$audit/enabled/Cargo.toml" \
  --format-version 1 \
  >"$audit/enabled/metadata.json"

metadata="$audit/enabled/metadata.json"
for package in opendal opendal-core; do
  count=$(jq --arg package "$package" '[.packages[] | select(.name == $package)] | length' "$metadata")
  if [[ $count -ne 1 ]]; then
    printf 'expected exactly one %s package, found %s\n' "$package" "$count" >&2
    exit 1
  fi
  jq -r --arg package "$package" \
    '.packages[] | select(.name == $package) | "resolved \(.name) \(.version)"' \
    "$metadata"
done

forbidden_opendal_features=(
  auto-register-services blocking executors-tokio internal-tokio-rt tests
  http-transport-reqwest http-transport-reqwest-native-tls
  http-transport-reqwest-rustls http-transport-reqwest-rustls-no-provider
  reqwest-rustls-no-provider-tls reqwest-rustls-tls
  layers-async-backtrace layers-await-tree layers-capability-check layers-chaos
  layers-concurrent-limit layers-dtrace layers-fastmetrics layers-fastrace
  layers-foyer layers-hotpath layers-immutable-index layers-logging
  layers-metrics layers-mime-guess layers-otel-metrics layers-otel-trace
  layers-prometheus layers-prometheus-client layers-retry layers-route
  layers-tail-cut layers-throttle layers-timeout layers-tracing
  services-aliyun-drive services-alluxio services-azblob services-azdls
  services-azfile services-b2 services-cacache services-cloudflare-kv
  services-compfs services-cos services-d1 services-dashmap services-dbfs
  services-dropbox services-etcd services-foundationdb services-foyer
  services-fs services-ftp services-gcs services-gdrive services-ghac
  services-github services-goosefs services-gridfs services-hdfs
  services-hdfs-native services-hf services-http services-huggingface
  services-ipfs services-ipmfs services-koofr services-lakefs
  services-memcached services-memory services-mini-moka services-moka
  services-mongodb services-monoiofs services-mysql services-obs
  services-onedrive services-opfs services-oss services-pcloud services-persy
  services-postgresql services-redb services-redis services-redis-native-tls
  services-rocksdb services-s3 services-seafile services-sftp services-sled
  services-sqlite services-surrealdb services-swift services-tikv services-tos
  services-upyun services-vercel-artifacts services-vercel-blob services-webdav
  services-webhdfs services-yandex-disk
)

for feature in "${forbidden_opendal_features[@]}"; do
  if jq -e --arg feature "$feature" '
    .packages[] as $package
    | select($package.name == "opendal")
    | .resolve.nodes[]
    | select(.id == $package.id)
    | .features
    | index($feature) != null
  ' "$metadata" >/dev/null; then
    printf 'forbidden OpenDAL feature enabled: %s\n' "$feature" >&2
    exit 1
  fi
done

for feature in blocking executors-tokio internal-tokio-rt reqsign services-memory; do
  if jq -e --arg feature "$feature" '
    .packages[] as $package
    | select($package.name == "opendal-core")
    | .resolve.nodes[]
    | select(.id == $package.id)
    | .features
    | index($feature) != null
  ' "$metadata" >/dev/null; then
    printf 'forbidden opendal-core feature enabled: %s\n' "$feature" >&2
    exit 1
  fi
done

jq -e '
  .packages[]
  | select(.name == "opendal-core")
  | [
      .dependencies[]
      | select(
          .name == "tokio"
          and .kind == null
          and .optional == false
          and (.features | (index("macros") != null and index("io-util") != null))
        )
    ]
  | length == 1
' "$metadata" >/dev/null || {
  printf 'opendal-core must have a mandatory normal Tokio edge with macros and io-util\n' >&2
  exit 1
}

for feature in macros io-util; do
  jq -e --arg feature "$feature" '
    .packages[] as $package
    | select($package.name == "tokio")
    | .resolve.nodes[]
    | select(.id == $package.id)
    | .features
    | index($feature) != null
  ' "$metadata" >/dev/null || {
    printf 'required Tokio feature missing: %s\n' "$feature" >&2
    exit 1
  }
done

for feature in rt rt-multi-thread; do
  if jq -e --arg feature "$feature" '
    .packages[] as $package
    | select($package.name == "tokio")
    | .resolve.nodes[]
    | select(.id == $package.id)
    | .features
    | index($feature) != null
  ' "$metadata" >/dev/null; then
    printf 'Tokio executor feature enabled: %s\n' "$feature" >&2
    exit 1
  fi
done

mkdir "$audit/disabled" "$audit/disabled/src"
cat >"$audit/disabled/Cargo.toml" <<EOF
[package]
name = "opendal-disabled-consumer"
version = "0.0.0"
edition = "2024"
rust-version = "1.92"

[dependencies]
typst-pack = { path = "${packaged[0]}", default-features = false }
EOF
cat >"$audit/disabled/src/main.rs" <<'EOF'
use typst_pack::opendal;

fn main() {
    let _ = std::any::type_name::<opendal::Operator>();
}
EOF

cargo metadata \
  --manifest-path "$audit/disabled/Cargo.toml" \
  --format-version 1 \
  >"$audit/disabled/metadata.json"
if jq -e '.packages[] | select(.name == "opendal" or .name == "opendal-core")' \
  "$audit/disabled/metadata.json" >/dev/null; then
  printf 'featureless packaged consumer retained an OpenDAL dependency\n' >&2
  exit 1
fi
if cargo check --manifest-path "$audit/disabled/Cargo.toml" \
  >"$audit/disabled/check.stdout" 2>"$audit/disabled/check.stderr"; then
  printf 'featureless packaged consumer unexpectedly exposed typst_pack::opendal\n' >&2
  exit 1
fi
disabled_error=$(<"$audit/disabled/check.stderr")
if [[ $disabled_error != *'unresolved import `typst_pack::opendal`'* \
  && $disabled_error != *'could not find `opendal` in `typst_pack`'* ]]; then
  printf 'featureless consumer failed for an unexpected reason:\n%s\n' "$disabled_error" >&2
  exit 1
fi

mkdir "$audit/compatibility" "$audit/compatibility/src"
cat >"$audit/compatibility/Cargo.toml" <<'EOF'
[package]
name = "opendal-compatibility-metadata"
version = "0.0.0"
edition = "2024"
rust-version = "1.92"

[dependencies]
opendal = { version = "0.58", default-features = false }
opendal-service-s3 = { version = "0.58", default-features = false }
EOF
cat >"$audit/compatibility/src/lib.rs" <<'EOF'
// Metadata-only compatibility probe.
EOF
cargo metadata \
  --manifest-path "$audit/compatibility/Cargo.toml" \
  --format-version 1 \
  >"$audit/compatibility/metadata.json"

for package in opendal opendal-core opendal-service-s3; do
  jq -e --arg package "$package" '
    [.packages[] | select(
      .name == $package and .edition == "2024" and .rust_version == "1.91"
    )] | length == 1
  ' "$audit/compatibility/metadata.json" >/dev/null || {
    printf '%s must resolve once with edition 2024 and rust-version 1.91\n' "$package" >&2
    exit 1
  }
  jq -r --arg package "$package" \
    '.packages[] | select(.name == $package) | "compatible \(.name) \(.version): edition \(.edition), Rust \(.rust_version)"' \
    "$audit/compatibility/metadata.json"
done

printf 'packaged OpenDAL compatibility audit passed for %s\n' "$workspace"
