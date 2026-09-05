#!/usr/bin/env bash
# CI provisioning only. The cargo command itself never installs toolchains.
set -euo pipefail
archive=$(mktemp)
trap 'rm -f "$archive"' EXIT
curl --fail --location --retry 3 \
  https://github.com/jart/cosmopolitan/releases/download/4.0.2/cosmocc-4.0.2.zip \
  --output "$archive"
printf '%s  %s\n' \
  85b8c37a406d862e656ad4ec14be9f6ce474c1b436b9615e91a55208aced3f44 \
  "$archive" | sha256sum --check
mkdir -p .cosmocc
unzip -q "$archive" -d .cosmocc
