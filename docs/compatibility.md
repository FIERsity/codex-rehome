# Compatibility

Supported in v0.1: macOS POSIX absolute paths, spaces, non-ASCII/NFC-equivalent component comparison, lexical `.`/`..`, child paths, missing old root during remap, and explicit distinction between lexical and canonical paths.

Case-insensitive filesystem identity is not inferred from string case. Cross-filesystem move is rejected. Windows drive paths, UNC, extended-length paths, and WSL aliases require a future Windows adapter; treating them as ordinary POSIX strings would be unsafe. Symlinks are recorded via real-path metadata but are not installed as compatibility shims.
