# Node assessment qualification

After reviewed source is committed, run:

```sh
cargo run -p rubix-dev --bin rubix-node-fixture --locked -- assessment capture /tmp/<unique-output>
cargo run -p rubix-dev --bin rubix-node-fixture --locked -- assessment verify /tmp/<unique-output>
cargo test -p rubix-dev --bin rubix-node-fixture --locked
```

This builds pinned Linux arm64 images and runs two owned, network-disabled,
nonroot containers. Only bounded private tmpfs paths are writable; no host paths
or devices are shared. A small shell adapter invokes the Rust fixture binary from
private `/usr/sbin`, retaining the original 1 MiB limit and invocation assertions.

The suite covers injected policy cases, eleven actual fixed-process cases,
a configured external-runtime assessment using injected host facts plus the real
probe, and four actual example exits. The external sentinel/config bytes must
survive every command probe. Ignored tests run only inside these disposable
containers. This evidence does not qualify rule programming or a node.

The Rust verifier requires schema 3, current source hashes, exact container argv,
nonce-bound builder/runtime binary identity, independently parsed raw observations,
namespace cleanup and repeated semantics. Raw command receipts must prove exit 0,
EOF, process-group absence and no timeout, cancellation or output overflow. Failed
cleanup cannot publish empty inventories. Late cancellation prevents success.

Historical raw evidence remains unchanged. The mandatory published-evidence test
fails until reviewed Rust captures replace it. No new capture was run during this
migration. Review and install each entire capture directory with its command
receipts; partial installation fails the exact file inventory check.

Install current Rust captures under `rust-evidence/`. Historical `evidence/`
remains unchanged. Image inspection and all cleanup operations have persisted
command receipts; empty inventories are checked against raw daemon output.
