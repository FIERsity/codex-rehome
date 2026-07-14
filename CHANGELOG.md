# Changelog

## 0.1.0-alpha.1 - Unreleased

- Initial defensive macOS CLI with inspect, plan, remap, move, verify, rollback, and doctor.
- Added deterministic plan IDs and unique UUID execution IDs.
- Locked mutation compatibility to the observed Codex 0.144.x SQLite schema and migrations 1–40.
- Added WAL-consistent SQLite online backups, backup tamper detection, post-write hashes, rollback locking, and restore verification.
- Added nested Desktop state traversal and a versioned, fully synthetic test fixture.
