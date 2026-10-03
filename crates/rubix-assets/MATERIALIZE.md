# Safe Asset Materialization (E06.02 / #49)

`Materializer` provides safe, atomic, and idempotent extraction of verified dependency
assets into configured writable host roots.

## Architecture and Safety Guarantees

1. **Path Safety and Isolation**:
   Every relative asset path is validated before creation. Path components containing
   traversal (`..`), empty components, Windows backslashes, drive-colons, or leading
   slashes are rejected with `MaterializationError::PathEscapesRoot` or
   `MaterializationError::InvalidRelativePath`. All destination paths must strictly
   reside within the caller-provided writable root.

2. **Staging and Atomicity**:
   Assets are never directly written or decompressed into their final destination paths.
   Extraction occurs in a private temporary staging directory
   (`<root>/.staging-<pid>-<timestamp>`) located within the same filesystem root.
   A RAII `StagingGuard` ensures that any failure or panic during extraction, decompression,
   or digest verification immediately removes the entire staging directory.
   Only when all bundled assets pass size, checksum, decompression, and on-disk
   reverification are files committed to their final destination via atomic `rename`.
   Failed extraction leaves zero usable-looking partial files at the destination.

3. **Disk Reverification Before Commit**:
   To prevent time-of-check to time-of-use (TOCTOU) corruption or partial disk writes,
   the bytes written to disk in staging are re-read and hashed with SHA-256 before any
   final placement. The observed on-disk size and SHA-256 must match the expected payload
   specifications exactly.

4. **Unix Permission Enforcement**:
   - Executables (`Kind::Executable`: `kube-apiserver`, `kubelet`, `containerd`, etc.) are
     materialized with Unix mode `0o755` (`rwxr-xr-x`).
   - Image payloads and archives (`Kind::Image`: `containerd/images/*.tar.gz`) are
     materialized with Unix mode `0o644` (`rw-r--r--`).

5. **Idempotence**:
   Calling materialization repeatedly on an already-populated root preserves file content
   and permissions. If existing files match the expected digest and size, redundant work
   is avoided while verifying and restoring correct permissions if tampered with.

6. **Streaming Tar/Gzip Archive Support**:
   A built-in streaming POSIX USTAR reader (`TarReader`) supports both uncompressed `.tar`
   and gzip-compressed `.tar.gz` distribution archives without introducing external
   runtime dependencies, enforcing member count and byte limits.
