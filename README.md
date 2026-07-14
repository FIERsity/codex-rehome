# Codex Rehome

Codex Rehome is a defensive CLI for inspecting, planning, remapping, moving, verifying, and rolling back local Codex Desktop/CLI workspace associations after a project directory changes. Codex stores absolute working-directory references in several local stores; moving files alone can leave threads grouped under the old path.

> Codex Rehome is an independent community project and is not affiliated with or endorsed by OpenAI.

## Status and scope

Version 0.1.0-alpha.1 targets macOS and the locally observed Codex CLI 0.144.x state shape (SQLite migrations 1–40). It is a prerelease: use synthetic or disposable projects only. It uses undocumented internal formats and deliberately refuses unknown SQLite schemas or malformed JSONL. A Codex update may therefore make it refuse to write until its adapter is reviewed. Linux is exercised in CI for portability; Windows/WSL path forms are documented but not yet supported.

`inspect` and `doctor` may read a newer schema when the minimal discovery columns remain available, but write compatibility is a separate exact gate. See the [compatibility matrix](docs/compatibility-matrix.md).

It does **not** migrate Python, R, Node, Conda or other environments; cloud tasks; data between machines; Git remotes; or conversation exports. It never creates a permanent compatibility symlink.

## Commands

- `inspect OLD`: read-only inventory of structurally associated threads and stores.
- `plan OLD NEW`: deterministic change inventory; add `--move-directory` for a move plan.
- `remap OLD NEW`: repair state after files were already moved.
- `move OLD NEW`: atomically rename the directory and repair state (same filesystem only in v0.1).
- `verify NEW --old OLD`: find structural old-path residue and confirm migrated threads.
- `rollback ID`: restore backed-up state and, when conflict-free, the directory.
- `doctor`: check the database schema and integrity.

All mutation commands are dry-run by default: they refuse to write without `--yes`. Close Codex Desktop and every Codex CLI before a write because live processes may overwrite files or hold SQLite WAL state.

```console
codex-rehome inspect /old/project
codex-rehome plan /old/project /new/project
codex-rehome remap /old/project /new/project       # refuses: dry-run
codex-rehome remap /old/project /new/project --yes
codex-rehome verify /new/project --old /old/project
```

For a tool-managed move, review `codex-rehome plan --move-directory /old/project /new/project`, then run `codex-rehome move /old/project /new/project --yes`.

Backups live under `$CODEX_HOME/rehome-backups/<migration-id>` with mode 0700 and files mode 0600. They may contain private task content and must be protected accordingly. On failure the tool restores state automatically. If manual recovery is needed, keep Codex closed and run `codex-rehome rollback <migration-id> --yes`; rollback refuses to overwrite a conflicting directory created after migration.

## 中文说明

Codex Rehome 用于项目目录移动或改名后，安全检查、规划、重映射、移动、验证并回滚 Codex Desktop/CLI 的本地路径关联。它只处理 Codex 本地状态，不迁移语言环境、依赖、云任务、Git remote 或跨机器数据，也不会自动创建永久符号链接。

`0.1.0-alpha.1` 针对 macOS 与已验证的 Codex CLI 0.144.x 内部状态（SQLite migration 1–40），仍是预发布版本，只应用于合成或可丢弃项目。内部格式并非公开 API，Codex 更新后工具可能为安全起见拒绝写入。所有写命令默认只演练，必须关闭 Codex 并显式加 `--yes`。备份位于 `$CODEX_HOME/rehome-backups`，可能包含敏感对话，权限设为 0700/0600。失败会自动恢复；也可在关闭 Codex 后运行 `codex-rehome rollback <migration-id> --yes`。

本项目是独立社区项目，与 OpenAI 无隶属关系，也未获 OpenAI 背书。

## Development

```console
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

All mutation tests must set `CODEX_HOME` to a temporary, synthetic fixture. Never test writes against a real `~/.codex`.
