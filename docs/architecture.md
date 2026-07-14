# Architecture

The CLI composes small adapters for SQLite, rollout JSONL, and allowlisted Desktop state. Discovery is read-only. Planning records exact files, field classes, counts, roots, thread IDs, warnings, and version data. Mutation occurs only after process checks, an exclusive lock, verified restricted-permission backups, and explicit confirmation. SQLite uses a transaction; JSON files use same-directory temporary files plus atomic rename. State is reloaded and verified, otherwise backups are restored.

Adapters fail closed on unknown required schema or malformed structured data. They never traverse arbitrary message text: rollout traversal is restricted to recognized path fields in record payloads, and Desktop traversal is restricted to recognized state keys.
