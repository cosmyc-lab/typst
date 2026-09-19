#!/usr/bin/env bash
# Run the CI gates that fail on code, locally, in the order CI runs them.
#
# `.github/workflows/ci.yml` exports `RUSTFLAGS=-Dwarnings` for every job,
# which is why a warning that is merely noisy on your machine is a hard
# error there — `cargo check` alone will never reproduce it. This script
# sets the same environment.
#
# CI runs the `tests`, `checks` and `docs` jobs on Rust 1.98; if your
# toolchain is older, a lint that CI reports may not exist for you yet.
#
# Left out, because they need a toolchain or tooling the workflow installs:
# the miri and fuzz jobs, the `docs` job (`cargo docit`), and the
# `min-version` job (same command as `check`, on Rust 1.92 — install it
# with `rustup toolchain install 1.92` and re-run with
# `RUST_TOOLCHAIN=+1.92 dev/ci-checks.sh check`).
#
# The `checks` job also ends with `git diff --exit-code`, to catch a
# command having rewritten a tracked file. That is not mirrored here: CI
# runs it on a clean checkout, whereas locally it cannot tell your own
# uncommitted edits from a file a gate rewrote, so it would always fail.
# `cargo fmt --check` above already covers the case that actually bites.
#
# Usage:
#   dev/ci-checks.sh              # everything below, stopping at the first failure
#   dev/ci-checks.sh clippy fmt   # only the named gates
#
# Gates: check, clippy, clippy-no-default, fmt, doc, test-build, test

set -uo pipefail

cd "$(dirname "$0")/.."

export RUSTFLAGS="${RUSTFLAGS:--Dwarnings}"
export RUSTDOCFLAGS="${RUSTDOCFLAGS:--Dwarnings}"
TC="${RUST_TOOLCHAIN:-}"

run() {
    local name=$1
    shift
    printf '\n\033[1m=== %s ===\033[0m\n%s\n' "$name" "$*"
    if "$@"; then
        printf '\033[32mok\033[0m   %s\n' "$name"
    else
        printf '\033[31mFAIL\033[0m %s\n' "$name"
        return 1
    fi
}

gate() {
    case $1 in
    # Mirrors the `min-version` job, which gained `--all-targets
    # --all-features` upstream in #8804.
    check) run check cargo $TC check --workspace --all-targets --all-features ;;
    clippy) run clippy cargo $TC clippy --workspace --all-targets --all-features ;;
    # Upstream replaced the single workspace-wide `--no-default-features`
    # run with one per crate that has cargo features: feature unification
    # across the workspace was masking errors.
    clippy-no-default)
        for p in typst-cli typst-kit typst-timing; do
            run "clippy-no-default($p)" cargo $TC clippy --package "$p" --no-default-features || return 1
        done
        ;;
    fmt) run fmt cargo $TC fmt --check --all ;;
    doc) run doc cargo $TC doc --workspace --no-deps --document-private-items ;;
    test-build) run test-build cargo $TC test --workspace --no-run ;;
    test) run test cargo $TC test -p typst-cnd ;;
    *)
        echo "unknown gate: $1" >&2
        return 1
        ;;
    esac
}

gates=("$@")
if [ ${#gates[@]} -eq 0 ]; then
    gates=(check clippy clippy-no-default fmt doc test-build test)
fi

for g in "${gates[@]}"; do
    gate "$g" || exit 1
done

printf '\n\033[32mall gates passed\033[0m\n'
