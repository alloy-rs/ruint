# Agent Guidelines

## Structure

- `src/`: core library and optional integrations.
- `ruint-macro/`: procedural macros.
- `ruint-bench/` and `benches/`: benchmarks.

## Commands

- Format: `cargo fmt --all`
- Lint: `cargo cl`
- Test: `cargo nextest run --workspace`

## Changelog

Add every user-facing change to `CHANGELOG.md` under `Unreleased`.
