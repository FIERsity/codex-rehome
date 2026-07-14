# Migration model

`MigrationPlan` format 1 records an ID, creation time, tool/Codex versions, lexical and real roots, operation, affected thread IDs, file/field/count changes, and warnings. The manifest adds checksummed backup mappings, status, and whether the directory moved.

Lifecycle: preflight → lock → stopped-process check → backup and checksum → optional same-filesystem rename → SQLite transaction and atomic JSON replacements → reload and verify → final manifest → unlock. Any error restores files and restores the directory only when doing so cannot overwrite another path.
