# Contributing

READZIP is a Rust workspace. A single `readzip` binary is built from `crates/readzip-cli`.

## Development

```bash
cargo test --workspace
cargo run -p readzip-cli -- demo
```

## Adding Language Support

Add detection and symbol extraction in `crates/readzip-core/src/lib.rs`, then add fixture coverage. Parser backend changes should preserve the same skeleton output shape.

## Commit Messages

Use Conventional Commits. Keep commit messages focused on the change and do not include generated-by footers or external-tool attribution.
