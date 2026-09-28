# Repository development tooling

Repository-owned executable tooling and tests use Rust. Do not add Python scripts,
inline Python programs, or Python test suites. Shell is appropriate for short command
sequences; parsing, validation, capture state machines and reusable logic belong in
the `rubix-dev` maintenance crate under `tools/dev`.

The maintenance crate is outside the node's runtime dependency graph and is not a
default workspace member. Tool binaries run explicitly through Cargo. Upstream code
generation remains an explicit maintenance operation, separate from ordinary builds.

Existing Python tooling is being migrated with its checks and failure behavior.
Deleting a verifier is not a migration. Preserve raw historical evidence and its
original source revisions; ported verifiers must distinguish historical integrity
from current-source qualification. New captures use the actual Rust tool revision.

Third-party source references remain read-only. External scanners and upstream tools
retain their own implementations; do not vendor their implementation into this repo.
