#!/usr/bin/env bash

TARGETS=(aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-gnu x86_64-unknown-linux-gnu)

for tgt in ${TARGETS[@]}
do
    echo "Building for $tgt"
    cargo build --release --target $tgt
done