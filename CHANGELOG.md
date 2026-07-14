# Changelog

## Unreleased

- Narrowed process detection to state-writing ChatGPT/Codex processes so the independent Codex Computer Use helper no longer blocks migrations.

## 0.1.0-alpha.1 - 2026-07-14

- Initial defensive macOS CLI with inspect, plan, remap, move, verify, rollback, and doctor.
- Added deterministic plan IDs and unique UUID execution IDs.
- Locked mutation compatibility to the observed Codex 0.144.x SQLite schema and migrations 1–40.
- Added WAL-consistent SQLite online backups, backup tamper detection, post-write hashes, rollback locking, and restore verification.
- Added nested Desktop state traversal and a versioned, fully synthetic test fixture.
- Added destination baselines for safe partially migrated workspaces.
- Added debug-only interruption testing and automatic restoration checks after SQLite and directory-move faults.
- Refused symbolic-link project roots and hard-linked state files during alpha writes.
- Added fsync-backed atomic rollout/Desktop/manifest replacement and retryable rollback interruption tests.
- Added cross-filesystem move preflight and explicit `failed-rollback-error` manifests when automatic recovery itself fails.
- Separated read-only schema discovery from the exact mutation gate and added a content-free `doctor` compatibility report.
- Added a version/platform compatibility matrix and future-schema read-only diagnostics.
- Added a release-binary disposable end-to-end exercise for remap, move, verification, and rollback on macOS and Linux CI.
- Added generated shell completions, a man page, checksum-verifying installation, and a three-platform prerelease workflow.
