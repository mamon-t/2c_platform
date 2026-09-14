#!/usr/bin/env bash
# Обёртка для rust-lld, запускаемая вместо прямого линкера на цели
# wasm32-unknown-unknown: убирает `-fuse-ld=*` (приносится глобальным
# ~/.cargo/config и не принимается rust-lld) и передаёт остальные
# аргументы настоящему rust-lld из sysroot.
set -u

HOST="$(rustc -vV | awk '/^host:/ {print $2}')"
SYSROOT="$(rustc --print sysroot)"
LLD="${LLD:-$SYSROOT/lib/rustlib/$HOST/bin/rust-lld}"

args=()
for a in "$@"; do
  case "$a" in
    -fuse-ld=*) ;;
    *) args+=("$a") ;;
  esac
done

exec "$LLD" "${args[@]}"