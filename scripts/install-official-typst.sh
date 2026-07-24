#!/usr/bin/env bash
set -euo pipefail

config=${1:?usage: install-official-typst.sh CONFIG DESTINATION}
destination=${2:?usage: install-official-typst.sh CONFIG DESTINATION}

read_pin() {
  local key=$1

  awk -v key="$key" '
    $0 == "[official-cli]" { in_section = 1; next }
    in_section && /^\[/ { exit }
    in_section && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" {
      value = $0
      sub("^[[:space:]]*" key "[[:space:]]*=[[:space:]]*\"", "", value)
      sub("\"[[:space:]]*$", "", value)
      print value
      found = 1
      exit
    }
    END { if (!found) exit 1 }
  ' "$config"
}

version=$(read_pin version)
url=$(read_pin url)
sha256=$(read_pin sha256)

[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] || {
  printf 'invalid official Typst version: %s\n' "$version" >&2
  exit 1
}
[[ $url == https://* ]] || {
  printf 'official Typst URL must use HTTPS: %s\n' "$url" >&2
  exit 1
}
[[ $sha256 =~ ^[0-9a-f]{64}$ ]] || {
  printf 'invalid official Typst SHA-256 digest\n' >&2
  exit 1
}

archive=$(mktemp)
trap 'rm -f "$archive"' EXIT

curl --proto '=https' --tlsv1.2 -LsSf "$url" -o "$archive"
printf '%s  %s\n' "$sha256" "$archive" | sha256sum --check
mkdir -p "$destination"
tar -xJf "$archive" -C "$destination" --strip-components=1

reported_version=$("$destination/typst" --version)
read -r binary actual_version _ <<<"$reported_version"
[[ $binary == typst && $actual_version == "$version" ]] || {
  printf 'official Typst version mismatch: expected %s, got %s\n' \
    "$version" "$reported_version" >&2
  exit 1
}
