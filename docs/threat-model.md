# Threat model

Protected assets include task history, private prompts, filesystem paths, and directory contents. Principal hazards are prefix collisions, overbroad text replacement, concurrent Codex writers, schema drift, partial writes, corrupt backups, malicious symlinks, and rollback overwriting new user data.

Controls include component-boundary matching, semantic allowlists, schema gates, explicit `--yes`, process refusal, exclusive locking, SQLite transactions, atomic same-directory JSON replacement, SHA-256 verification, 0700/0600 backup permissions, post-write reload, automatic restoration, conflict-safe rollback, symbolic-link root refusal, and hard-link refusal for state files. Debug builds provide deterministic interruption points used only by synthetic tests; release builds ignore the fault-injection variable. Backups remain sensitive and should be deleted only after the user is satisfied with recovery.
