# How READZIP Works

READZIP uses a Claude Code `PreToolUse` hook for native `Read`.

1. The agent asks to read a large file without `offset` or `limit`.
2. READZIP parses the file into a compact structural skeleton.
3. READZIP returns a structured deny response with the skeleton in `permissionDecisionReason`.
4. The agent reads the skeleton and retries with a scoped `Read(offset, limit)`.
5. Scoped reads pass through unchanged.

READZIP intentionally avoids `updatedInput`; current hook implementations do not apply it consistently enough for transparent rewrites.

Recent Claude Code versions also accept `additionalContext` from `PreToolUse`. READZIP includes a short session hint there so the agent can learn the skeleton-then-section workflow, but the core behavior does not depend on it. The skeleton itself stays in `permissionDecisionReason`.

The deny-reason skeleton defaults to roughly 1,500 tokens via `max_skeleton_tokens`. This is intentionally conservative: very long hook denials are harder for models to parse cleanly. Heavy users can raise the setting, but staying under about 2,500 tokens is recommended.
