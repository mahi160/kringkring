#!/bin/sh
set -eu

target=${1:-}
case "$target" in
    x86_64-unknown-linux-musl)
        platform=linux/amd64
        builder='rust:alpine3.24@sha256:cfbeb9c8c956f048a081ae034a7c8ffbd60242f3ff76b645d3a41523f665f761'
        runtime='alpine:3.24@sha256:0f49f331d9212aff11b932863be4eb01cdf962969aa86dab7d1ee21ac6f5f52b'
        ;;
    aarch64-unknown-linux-musl)
        platform=linux/arm64
        builder='rust:alpine3.24@sha256:18b6dc7bc111bde4e2fccf6c4ec95cc4a3486b41e4b090e9b0202eba9540023e'
        runtime='alpine:3.24@sha256:1829893486be257df5049ab3b7fc95afbd0fc09d89803cbc580bda1ba5386b29'
        ;;
    *) printf 'Usage: %s {x86_64,aarch64}-unknown-linux-musl\n' "$0" >&2; exit 2 ;;
esac
build_dir="target/release-$target"
asset="kringkring-$target"

docker run --rm --platform "$platform" -v "$PWD:/work" -w /work \
    -e RUSTFLAGS='-C target-feature=+crt-static' \
    -e CARGO_TARGET_DIR="/work/$build_dir" "$builder" \
    cargo build --locked --release --target "$target"

binary="$build_dir/$target/release/kringkring"
description=$(file "$binary")
printf '%s\n' "$description"
case "$description" in
    *'statically linked'*|*'static-pie linked'*) ;;
    *) printf 'Release binary is not static\n' >&2; exit 1 ;;
esac

docker run --rm --network none --platform "$platform" \
    -v "$PWD/$binary:/usr/local/bin/kringkring:ro" "$runtime" sh -eu -c '
        kringkring init --data-dir /tmp/kringkring --username smoke --pin 0123
        kringkring serve --data-dir /tmp/kringkring &
        for attempt in 1 2 3 4 5 6 7 8 9 10; do
            if wget -q -O /dev/null http://127.0.0.1:8080/health; then exit 0; fi
            sleep 1
        done
        printf "Alpine health check did not return success\n" >&2
        exit 1
    '

mkdir -p dist
cp "$binary" "dist/$asset"
(
    cd dist
    sha256sum "$asset" > "$asset.sha256"
)
