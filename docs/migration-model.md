# Migration model

`MigrationPlan` format 2 has a deterministic plan ID derived from its semantic JSON. It records tool/Codex versions, schema fingerprint, lexical and real roots, operation, affected thread IDs, file/field/count changes, and warnings. Each execution receives a separate UUID migration ID. The manifest adds creation time, source/snapshot/post-write checksums, backup mappings, status, and whether the directory moved.

Lifecycle: preflight → stopped-process check → lock → WAL-consistent SQLite snapshot and checksummed file backups → optional same-filesystem rename → SQLite transaction and atomic JSON replacements → exact-count reload and verification → post-write checksums → final manifest → unlock. Any error restores files and restores the directory only when doing so cannot overwrite another path. Rollback independently acquires the same lock, verifies backups before use, and verifies restored structural references afterward.
