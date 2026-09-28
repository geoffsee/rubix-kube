# Declared platform manifest qualification

This offline fixture binds a caller-declared DockerV2 platform manifest to a completed
crane Docker-save archive/layer observation. It checks exact raw manifest/config bytes,
ordered stored layer descriptors and complete decoded DiffIDs. It does not authenticate
a publisher, select an index entry, approve production asset pins, import an image, or
interpret or execute layer contents.

`inputs.json` records the CoreDNS 1.14.4 Linux arm64 bytes retained from the registry at
2026-09-28T05:09:35.955824+00:00. The small raw manifest and config are retained here;
the thirteen compressed blobs total 21,174,946 bytes and remain in an external cache.
The raw platform-manifest digest is
`sha256:18f34dc0909c41ffc499684a992898376c58d0ec8460d59ce676a35a0df5ccd0`.
The separately observed index digest was
`sha256:3e98f280fd601b37411c5fb7075fd9f337833c480f1644970b727ae0af067782`;
that index is provenance, not an acceptance input.

The pinned Go 1.26.2 / go-containerregistry v0.21.5 producer returns the retained raw
config and original compressed blobs through `partial.CompressedImageCore`, writes
`crane.Save` with tag `coredns-fixture:manifest-binding`, then applies Go's default
single gzip frame (zero modification time). Two offline packaging runs were identical.
An independent Ruby Zlib/Gem TarReader audit checked all fifteen ordered regular tar
members, exact config and compressed bytes, framing closure and all thirteen DiffIDs.
`archive-pin.json` records that fixture-only encoded archive identity: 21,071,411 bytes,
SHA-256 `546ffb720afb37c1c537c5113c366b9cd3f7a714a4f80e5c3ca11843eedc6124`.
Hardening input opens and output ownership preserved those exact archive bytes.

## Prepare the immutable input cache

The existing retained cache can be checked directly:

```sh
cargo build -p rubix-dev --bin rubix-asset-fixture --release --locked
target/release/rubix-asset-fixture manifest-inputs /path/to/cache
```

For a fresh cache, run the following from the repository root with Bash, curl and jq.
Choose a nonexistent `manifest_cache` path. This retrieves only immutable blob digests;
it never resolves a tag. The manifest/config are copied byte-for-byte from this fixture.
Do not enable shell tracing. The temporary bearer token/header remain private, are
never printed, and are removed on exit. `curl --output` preserves raw response bytes;
do not substitute a CLI that prints a manifest with an appended newline.

```bash
set -euo pipefail
set +x
manifest_cache=/tmp/rubix-coredns-manifest-cache
mkdir -m 700 "$manifest_cache"
mkdir "$manifest_cache/blobs"
manifest_auth=$(mktemp -d)
trap 'rm -f "$manifest_auth/token.json" "$manifest_auth/header"; rmdir "$manifest_auth"' EXIT
trap 'exit 1' HUP INT TERM
chmod 700 "$manifest_auth"
curl --fail --silent --show-error --proto '=https' --max-time 60 \
  --output "$manifest_auth/token.json" \
  'https://auth.docker.io/token?service=registry.docker.io&scope=repository:coredns/coredns:pull'
jq -er '"Authorization: Bearer " + (.token | select(type == "string" and length > 0))' \
  "$manifest_auth/token.json" > "$manifest_auth/header"
cp tools/assets-manifest/arm64-manifest.json "$manifest_cache/arm64-manifest.json"
cp tools/assets-manifest/arm64-config.json "$manifest_cache/arm64-config.json"
while IFS=$'\t' read -r manifest_digest manifest_size; do
  curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' \
    --max-time 180 --max-filesize "$manifest_size" --header @"$manifest_auth/header" \
    --output "$manifest_cache/blobs/$manifest_digest" \
    "https://registry-1.docker.io/v2/coredns/coredns/blobs/sha256:$manifest_digest"
done < <(jq -r '.layers[] | [.stored_sha256, .stored_bytes] | @tsv' tools/assets-manifest/inputs.json)
target/release/rubix-asset-fixture manifest-inputs "$manifest_cache"
```

These commands were cold-tested into a fresh external cache; all thirteen immutable
downloads passed the Rust checker. The Rust checker requires exact hashes/sizes and independently bounded complete gzip
DiffIDs. A partial download or changed byte is rejected; a failed cache is not qualified.
Capture verifies the source cache, copies only listed regular files into an owned build
context, and verifies the copy again. The producer uses nonblocking, no-follow regular
file opens and creates a new private output directory before serialization.

## Capture and verify

After source review and a clean source freeze:

```sh
target/release/rubix-asset-fixture manifest capture /tmp/new-manifest-evidence /path/to/cache
target/release/rubix-asset-fixture manifest verify /tmp/new-manifest-evidence
```

The shared schema-3 capture infrastructure binds the source inventory, nonce-delimited
three-binary build, image inspection, exact settled commands/raw logs and owned cleanup.
Both runs use UID 65532, no network, read-only image, no capabilities, a private 96 MiB
tmpfs and bounded processes/memory/time. Large payloads stay in files; logs contain only
bounded metadata and independent observations. The consumer checks the actual CoreDNS
positive, nineteen identity/semantic/limit negatives (rehashing the declared manifest
for semantic negatives), retained budgets, and eight synthetic profile regressions.
Successful binding emits its config/layer observations for comparison to the independent
oracle. The synthetic cases cover OCI gzip/zstd and ordered repeated references.

Publish only verified current evidence at `rust-evidence/`; the mandatory current gate
requires fresh source-bound Rust receipts. Historical acquisition facts and fixture pins
do not replace current capture proof. The verifier needs no registry access or external
payload cache after publication: its expected byte identities are pinned in source and
were independently checked against the runtime inputs before consumer execution.
