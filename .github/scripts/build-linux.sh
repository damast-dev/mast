#!/bin/sh
# Builds the static (musl) mast binary and its .deb for the host architecture. Runs inside rust:1-alpine:
#   docker run --rm -v "$PWD:/src" -w /src -e VERSION=0.7.0-mast.1 rust:1-alpine .github/scripts/build-linux.sh
# Output: target/<triple>/release/mast, dist/mast_<deb-version>_<arch>.deb
set -eu
: "${VERSION:?set VERSION, e.g. 0.7.0-mast.1}"
DEB_VERSION=$(echo "$VERSION" | sed 's/-rc\./~rc./') # RCs sort before their final release in apt

apk add --no-cache musl-dev openssl-dev openssl-libs-static zlib-dev zlib-static pkgconf >/dev/null
target=$(rustc -vV | sed -n 's/^host: //p')
export CARGO_PROFILE_RELEASE_STRIP=true OPENSSL_STATIC=1 PKG_CONFIG_ALL_STATIC=1 BUILD_TYPE="mast v$VERSION release"
cargo build --locked --release --target "$target"
cargo install --locked --quiet cargo-deb --version 3.8.0
mkdir -p dist
cargo deb --no-build --locked --target "$target" --deb-version "$DEB_VERSION" --output dist/
