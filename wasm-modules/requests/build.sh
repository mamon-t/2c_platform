#!/usr/bin/env sh
# Сборка WASM-плагина.
# RUSTFLAGS="" перекрывает глобальный ~/.cargo/config.toml
# (-fuse-ld=lld ломает rust-lld для wasm32 после обновления тулчейна).
set -e
RUSTFLAGS="" cargo build --target wasm32-unknown-unknown --release --manifest-path "$(dirname "$0")/Cargo.toml" "$@"
