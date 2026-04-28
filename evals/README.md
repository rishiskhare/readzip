# Evals

Two layers, ordered by how reproducible the numbers are:

## Layer 1 — `readzip eval <dir>` (static corpus eval)

Walks a directory, generates a skeleton for every supported source file over the `min_lines` threshold, and reports total tokens before/after. Reproducible from a clean clone in seconds.

```bash
cargo build --release -p readzip-cli

# This repo's own crates (the dogfood):
./target/release/readzip eval crates/

# Any directory:
./target/release/readzip eval ~/code/some-project/

# JSON for dashboards / CI:
./target/release/readzip eval ~/code/some-project/ --json
```

Output:

```
# readzip eval

- target(s): crates/
- min_lines threshold: 500
- files scanned: 5
- files intercepted: 2
- total original tokens: 19.1K
- total skeleton tokens: 2.7K
- tokens saved: 16.4K
- average reduction: 85.9%

| File | Lang | Lines | Original | Skeleton | Saved | Reduction |
|---|---|---:|---:|---:|---:|---:|
| crates/readzip-cli/src/main.rs | Rust | 1323 | 11422 | 1461 | 9961 | 87.2% |
| crates/readzip-core/src/lib.rs | Rust | 924  | 7723  | 1248 | 6475 | 83.8% |
```

This isn't an end-to-end agent eval — it measures the savings on a single full-file Read. Real agent sessions usually re-read the same files multiple times, so wall-clock context savings are typically *larger* than this metric suggests. But this is the floor.

## Layer 2 — Task-based agent eval (planned, not yet shipped)

The launch claims (~81% on big files, agents completing tasks in fewer turns) need to be backed by real LLM-driven evals across multiple repos and arms. The structure that lands next:

```
evals/
├── projects/                    git submodules of real OSS repos
│   ├── small-rust-cli/          ~5K LOC
│   ├── medium-ts-app/           ~30K LOC
│   └── large-py-monorepo/       ~100K+ LOC
├── tasks/                       canned prompts per project
│   ├── small/fix-rate-limiter.md
│   ├── medium/refactor-auth.md
│   └── ...
├── runner/                      driver that runs each task in 3 arms
│                                — baseline / readzip / serena
└── results/                     committed markdown tables
```

Each task gets run in three arms with the same prompt and the same model; the runner records input tokens per turn, total tokens per task, turns to completion, and pass/fail.

This layer requires API keys, deterministic seeding, and project / task curation — contributions welcome. For now, Layer 1 plus `readzip stats` from real Claude Code sessions is the empirical evidence the project ships with.

## Smoke test

```bash
cargo run -p readzip-cli -- demo --json
```

Generates a skeleton from a bundled fixture file and emits stable JSON suitable for dashboards / CI. This is what `.github/workflows/test.yml` runs on every push.
