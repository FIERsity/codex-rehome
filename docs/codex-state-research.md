# Codex local-state research (2026-07-14)

Evidence labels are intentional; they must not be collapsed into a single claim.

## Officially documented

OpenAI's public Codex documentation describes local CLI configuration and resuming sessions, but the reviewed documentation does not publish a supported workspace-remap command or a stable contract for Desktop's local persistence files. Therefore this tool treats these formats as private and version-sensitive.

## Confirmed from openai/codex source

The official repository is the source of the CLI/state runtime. Source and issue traces establish a SQLite state runtime with migrations, WAL/busy handling, thread `cwd`, and rollout paths. Before each compatibility release, maintainers must pin links/commits and compare migrations and write paths; a GitHub issue report alone is not source confirmation.

## Read-only local observation

On macOS with `codex-cli 0.144.2`, a read-only inspection found:

- `~/.codex/state_5.sqlite`, with a `threads` table containing `id`, `cwd`, and `rollout_path` (174 rows at inspection time);
- `session_index.jsonl`, whose observed index keys were `id`, `thread_name`, and `updated_at`;
- rollout JSONL records whose structured `session_meta.payload` includes `cwd`, and later `turn_context` records;
- `.codex-global-state.json` keys including active/saved workspace roots, project order, thread workspace-root hints, and writable roots.

No conversation body was copied into the repository. This observation is a snapshot, not an API promise.

## Community observations

Open issues #15347, #21076, #23979, #24178 and related reports describe threads disappearing after moves or updates, multiple path spellings, stale indexes, and disagreement between SQLite, rollout files, and Desktop state. The “Codex project history path migration guide” Gist and projects such as Codex Thread Mover/path migrators demonstrate demand, but their field lists and replacement strategies are not authoritative.

## Unverified hypotheses

- Which store is authoritative for every Desktop release.
- Whether every allowlisted Desktop key is required for grouping.
- Whether sandbox policy strings can safely be migrated; v0.1 does not mutate them.
- Whether session indexes should be rewritten or rebuilt; v0.1 leaves them untouched unless a future source-backed adapter is added.

Sources: [official Codex documentation](https://developers.openai.com/codex/), [openai/codex](https://github.com/openai/codex), [workspace remap issue #15347](https://github.com/openai/codex/issues/15347), [state inconsistency issue #21076](https://github.com/openai/codex/issues/21076), [missing history issue #23979](https://github.com/openai/codex/issues/23979), [stale path issue #24178](https://github.com/openai/codex/issues/24178), and [community migration guide](https://gist.github.com/lyzno1/cd8d4c86fa843894ea7cde4156651b7b).
