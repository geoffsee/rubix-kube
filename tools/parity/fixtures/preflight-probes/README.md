# Disposable supplemental probe qualification

This fixture runs actual Rust supplemental filesystem and wildcard socket probes
in fresh Linux/arm64 Docker containers. It is not a new Go oracle: the existing
`../preflight-policy` fixture independently captured the pinned Go disk/glob and
mandatory/pprof port behavior. The Rust production source and private-root test
adapter are separately identified by `source-hashes.json`.

The build pins the Rust image by digest, Cargo.lock and the repository toolchain.
Each of two fresh containers runs the pure tests plus two explicitly ignored live
cases, as UID 65532, without capabilities/network access/host mounts, on a read-only
root with a 16 MiB private tmpfs. The network namespace permits real IPv4 and IPv6
wildcard listeners without affecting host services. No sysctl, module load or
package/service command runs. Private module files exist only in an owned temporary
root used by the test's filesystem adapter. Public `collect_supplemental` is also
called read-only against the container namespace; host-specific output is not
normalized into a success oracle.

Socket records prove free ports, one-family-only conflicts, disabled pprof skipping,
closure/rebind and preservation of competing listeners. The explicitly labeled
`injected_ipv6_unsupported_real_ipv4` case injects an initial unsupported-family
result and then really binds/conflicts/closes IPv4. Detection on a kernel without
IPv6 remains unqualified; no host IPv6 setting is changed.

The Rust verifier requires a current schema 3 receipt. Historical evidence remains
unchanged. Publish a separate verified Rust capture from reviewed source.
Run these commands from a clean, committed checkout after source review:

```sh
cargo run -p rubix-dev --bin rubix-node-fixture --locked -- probes capture /tmp/new-probe-evidence
cargo run -p rubix-dev --bin rubix-node-fixture --locked -- probes verify /tmp/new-probe-evidence
cargo test -p rubix-dev --bin rubix-node-fixture --locked
```

Review and install the entire capture directory together; each command receipt is
part of the evidence. The `published_probes_require_current_rust_capture` gate
checks the installed `rust-evidence/` directory against current source hashes.

The build is bounded to 900 seconds/8 MiB, each run to 60 seconds/1 MiB, and
metadata/cleanup commands to 30 seconds/64 KiB. Builder nonce frames bind the exact
binaries to both executions. Raw bytes and settled command receipts prove exit
status, EOF, process-group absence, and absence of cancellation, timeout, or output
overflow. The independent oracle checks every filesystem/socket marker and all
four successful test summaries. Repeated observations must agree.

Only the capture's exact owned image and containers are removed. Uncertain process
cleanup stops further effects and leaves inventory unknown. Failed daemon queries
cannot claim empty inventories. Cancellation is sampled again before publication.

Install new captures under `rust-evidence/`, preserving historical `evidence/`.
Image identity and empty cleanup inventories require their own persisted raw
command receipts and exact daemon argv.
