# AGENTS.md

Guidance for AI coding agents working in this repository.

## Project Overview

ruint provides unsigned integers with const-generic bit widths. The core library
supports `no_std`, with optional features for allocation, randomness,
serialization, database, and ecosystem integrations.

## Commands

```bash
cargo build --workspace                         # Build
cargo nextest run --workspace                   # Test
cargo fmt --all                                 # Format
cargo clippy --workspace --all-features --all-targets # Lint
cargo check --no-default-features               # Check no_std support
```

## Architecture

- `src/`: core library and optional integrations.
- `src/algorithms/`: unstable low-level arithmetic algorithms.
- `src/support/`: feature-gated ecosystem integrations.
- `ruint-macro/`: procedural macros.
- `ruint-bench/` and `benches/`: benchmarks.

## Testing

- Keep unit tests alongside the implementation being changed.
- Use the existing test and benchmark infrastructure; do not add standalone
  test files.
- Use `const_for!` when checking behavior across supported bit widths.

## Changelog

- Add every user-facing change to `CHANGELOG.md` under `Unreleased`.
- Follow Keep a Changelog categories: `Added`, `Changed`, `Deprecated`,
  `Removed`, `Fixed`, or `Security`. Create the relevant subsection if needed.
- Write one concise bullet per change, without a trailing period.
- Once a PR number exists, append `([#NNN])` to the bullet and add
  `[#NNN]: https://github.com/alloy-rs/ruint/pull/NNN` with the other
  `Unreleased` link definitions.
- Do not create a versioned release section as part of a normal change.
