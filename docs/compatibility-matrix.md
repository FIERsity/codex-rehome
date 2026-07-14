# Compatibility matrix

Codex Rehome separates read compatibility from write compatibility. A schema with the three discovery columns (`id`, `cwd`, and `rollout_path`) can be inventoried without granting mutation support. Mutation requires an exact, tested adapter match.

| Platform | Codex surface | Observed version | State adapter | Read | Write | Evidence |
|---|---|---:|---|---|---|---|
| macOS arm64 | CLI + Desktop | CLI 0.144.2 | `codex-state-0.144-v40` | Yes | Alpha | Local read-only schema audit; synthetic migration/rollback suite |
| Linux | CLI-shaped synthetic state | 0.144 schema fixture | `codex-state-0.144-v40` | Yes | Test only | GitHub Actions on Ubuntu |
| Windows native | Desktop + CLI | Not audited | None | No | No | Design only |
| WSL | Desktop agent mode | Not audited | None | No | No | Design only |

“Alpha” means mutation is implemented and tested only against disposable/synthetic projects. It does not authorize migration of an important real project.

## Adapter gate

`codex-state-0.144-v40` requires:

- the exact validated 32-column `threads` layout;
- exactly 40 successful SQLx migrations with maximum version 40;
- a passing SQLite integrity check for `doctor` compatibility;
- recognized structured rollout and Desktop state JSON.

`doctor` emits a content-free JSON report containing platform, tool/Codex versions, store presence, column names, migration summary, schema fingerprint, integrity result, and incompatibility reasons. It never emits thread IDs, titles, paths from thread rows, or conversation bodies.

## Updating the matrix

For every new Codex release:

1. Close Codex for any mutation test; initial audit remains read-only.
2. Run `codex-rehome doctor` and save only its JSON report.
3. Compare columns, migration count/max, and fingerprint with the last validated adapter.
4. Review corresponding `openai/codex` state migrations and path write sites.
5. Create a new synthetic fixture that preserves shape but contains no user content.
6. Add read-only, mutation, interruption, verification, and rollback tests.
7. Add a new adapter only after CI and a disposable-project macOS validation pass.
8. Update this matrix with evidence; never widen an existing gate merely to accept a new fingerprint.
