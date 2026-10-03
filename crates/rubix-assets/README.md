# Declared dependency inventory and encoded-byte checks

This is the first bounded layer of E06.01/#48. It is a synchronous, read-only
library shared by future runtime and release consumers. It does not open paths,
fetch inputs, start commands, parse general OCI content, install
files, import images or authorize any of those actions.

`Manifest::decode(bytes, limits)` is the sole public construction route and parses
the strict schema. The opaque wrapper does not implement serde Deserialize. It
retains the exact original byte length, including whitespace, and rechecks that
length if validation supplies tighter limits. Unknown/duplicate fields,
unknown enum values and invalid JSON fail. `validate_inventory(request, limits)`
checks schema version1, the requested `rubix_platform::NodeTarget`, variant/scope,
exact required roles, permitted delivery/encoding, unique safe relative paths,
hex digests and size budgets. It returns `DeclaredInventory`: **declared metadata**,
not authenticated content, provenance or release readiness. Its immutable iterators
expose sorted role/delivery records and bundled metadata to E27 without duplicating
the component list.

`optional_feature_support()` separately reports target-policy support for local-path
storage, the default Portainer agent and D2K. `SupportedTarget` is not an enabled
feature or proof of a pinned payload; E27 must combine it with the declared role
delivery and the selected runtime configuration. The offline contract bundles every
supported optional image (including the local-path helper) while marking only
target-restricted Portainer/D2K roles unavailable. CoreDNS and pause remain bundled
in both variants.

Release consumers must decode and validate the complete manifest for the requested
target/variant before performing installation side effects. Wrong-target, missing-role,
malformed and truncated manifest inputs are rejected by this same API. Validation does
not authenticate otherwise well-formed manifest bytes, authorize installation, or
replace verification of the exact bytes later materialized. Linux ARM64/glibc fixtures
are structural manifests: Kubernetes arm64 pins match published releases, and the
remaining bundled digests are placeholders rather than an accepted production cell.

`DeclaredInventory::verification_session()` creates an aggregate encoded-read
budget. `verify_encoded_blob(id, reader)` checks exact encoded length and SHA-256,
returning `EncodedBlobMatch`. It reads no more than declared length+1 and uses an
8KiB buffer. Short reads work; I/O errors retain their original `std::io::Error`.
An absent/unbundled role fails without reading. Attempts reserve declared length+1
before reading, including failed/repeated attempts; exhausted sessions cannot
perform additional reads. A new session is an explicit new caller-owned budget.
No aggregate-all-assets success token is produced.

Neither type proves decompressed integrity, an actual executable's architecture or
libc, image platform/digests, archive safety, provenance authenticity or that bytes
are executable. The synthetic `abc` fixture intentionally matches encoded hashes
for declared ELF/gzip/zstd roles without being any of those formats. This is a
regression against overstating the API's guarantee, not a production asset lock.
Later content inspection is required before #48 closes. E06.02 must reverify bytes
actually copied into its owned temporary output before committing; an earlier
encoded match does not prevent later mutation of a reopened path.

## Schema and scope

See [the independent fixture](tests/fixtures/online-amd64.json) for the exact JSON
shape. Its digests are real SHA-256(`abc`) test vectors. Do not ship it. Fields are:

- `schema_version`, `target:{os,architecture,libc}`, `variant`, `scope`, `assets`.
- `os` is `linux`; architectures are `amd64`, `arm64`, `armv7`, `riscv64`; libc is
  `glibc` or `musl`. The requested target uses the existing platform type.
- `variant` is `online` or `offline`. `scope` is `supervised-bundle` or
  `legacy-external-deps`.
- Every supervised-bundle role occurs once. `delivery.kind` is `bundled`,
  `registry-required` or `unavailable`. Bundled records additionally require `path`,
  `encoding`, positive `encoded_bytes` and exactly64 lowercase hexadecimal `sha256`
  characters. Nonbundled variants permit no extra fields.
- Paths are canonical ASCII POSIX relative names: at most32 nonempty components,
  using letters/digits/dot/dash/underscore, excluding `.` and `..`. Absolute,
  trailing/repeated separator, backslash, drive-colon, control and Unicode names
  are rejected. Equal paths and file/descendant conflicts are rejected. This is
  content-store metadata, not permission to join a path to an arbitrary host root.

`legacy-external-deps` accepts only an empty payload list. It describes the pinned
Go build tag's absence of embedded bytes. It does **not** qualify a complete
supervised-node executable inventory or extend host-supplied semantics to new
components. Its distinct scope remains observable in `request()`. External CRI
ownership and runtime activation are separate E06.03/E09/E10 concerns; this library
never transfers ownership of host daemons, binaries or images.

## Limits and errors

Defaults are explicit engineering policy:1MiB input JSON,256 records,4096 path
bytes,8GiB per encoded blob and32GiB per verification session. Aggregate validation
reserves one trailing-byte/EOF probe per bundled record. JSON input bounds total
parse allocation; record/path caps are checked after typed decoding. Custom limits
must be positive; the per-asset maximum must leave room for a checked +1. Aggregate
addition and session subtraction are checked. These numbers are not measurements
of production asset requirements and should be reviewed when real payloads arrive.
A blocking `Read` implementation's time/cancellation behavior belongs to its caller;
byte bounds are not a wall-clock deadline. Interrupted reads are returned as I/O
errors, not retried without a bound.

JSON errors expose line/column without echoing input. Inventory errors include
fixed role IDs and structured kinds. Verification errors preserve I/O sources but
do not include raw source messages in Display. Callers decide how to surface them.

## Validation

`cargo test -p rubix-assets --locked`, the release equivalent, strict package Clippy
and package formatting checks cover16 target/variant cells, unsupported-image
restrictions, legacy scope, schema/duplicate/path/hash/size failures, wrong declared
targets, fixed independent SHA-256 vectors, short/error/misbehaving readers,
truncation/corruption/trailing bytes, and aggregate budget exhaustion.

[CATALOG.md](CATALOG.md) records source authority, actual unresolved production pins,
encodings, the full matrix and the remaining content-validation/asset-resolution
gates. Ordinary builds perform no upstream refresh and embed no third-party assets.

The additive [identity ELF inspection](ELF.md) now checks the exact identity bytes and
reports bounded header/loader/dependency facts. Runtime ABI closure remains
separate; this addition does not close #48.

The additive [bounded compressed-byte inspector](DECODE.md) now streams one zstd
frame or gzip member with explicit budgets. Its decoded digest/count observations
do not establish ELF/archive/OCI semantics or authenticate a publisher.

`DecodeSession::inspect_compressed_elf` now composes complete compressed executable
inspection with the same bounded ELF parser, using a private provisional buffer
and retained aggregate budgets. Its combined observation still leaves production
decoded pins, archive/OCI content, ABI qualification and materialization open.

The additive [single-image crane archive inspector](ARCHIVE.md) streams gzip/tar
structure and stored-member digests with bounded metadata. Config platform and
reference observations alone leave nested layer codecs/DiffIDs, original registry
manifest identity, production pins and safe materialization unresolved. The
stronger methods below add separate checks without changing that API's guarantee.

The additive [nested layer digest verifier](LAYER-INTEGRITY.md) now matches complete
bounded gzip/zstd layer output hashes against ordered declared DiffIDs, retaining
the same session budgets. This is a byte-stream identity proof; inner tar safety,
unsupported codecs, production payload provenance and import remain separate.

The additive [declared platform-manifest binding](MANIFEST-BINDING.md) consumes a
completed layer observation and matches its encoded archive, config, ordered
stored-layer descriptors, and Linux platform to a caller-declared raw manifest
pin. The opaque result proves byte agreement with that declaration. Registry/index
selection, publisher authentication, production pin approval, ABI compatibility,
inner-tar safety, and installation eligibility remain outside the proof.

## Variant matrix and artifact naming (E27.01/#114)

`Matrix` defines the exhaustive 16 node variant cells (4 architectures: amd64, arm64,
ARMv7 hard-float, riscv64 × 2 libcs: glibc, musl × 2 variants: online, offline) and
the 4 supported management CLI targets (Linux amd64/arm64, Darwin amd64/arm64).
Per E01 and the compatibility contract, native Windows binaries are explicitly excluded,
with WSL2 supported via Linux userspace.

`NodeVariant` models cell index (1..=16), architecture, libc, variant, OCI container image
platform string (`linux/amd64`, `linux/arm64`, `linux/arm/v7`, `linux/riscv64`), optional
image restrictions (Portainer supported on amd64/arm64/armv7, unsupported on riscv64;
D2K supported on amd64/arm64, disabled on armv7 and riscv64; local-path supported on all),
and whether optional images are bundled (false for online, supported for offline).

`ArtifactNaming` provides reproducible parsing and clean-checkout input validation for:
- Node archives: `<prefix>-<version>-linux-<arch>[-musl][-offline].tar.gz`
- Management binaries: `<prefix>-<os>-<arch>`

Validation requires version format, target, libc, variant, and asset naming consistency
before release assembly or installation.

## Safe asset materialization (E06.02/#49)

The additive [safe asset materializer](MATERIALIZE.md) extracts verified dependency assets
into configured writable host roots.

- **Path Safety**: Enforces containment within the configured writable root. Directory
  traversal (`..`), empty path segments, Windows drive colons, and leading slashes are rejected.
- **Staging and Transactions**: Private descriptor-relative staging, destination preflight,
  a per-root lock and retained originals allow synchronous commit errors to roll back new
  files and restore the previous installation. Rollback failures retain recovery backups
  and are explicit errors. Individual renames are atomic; this is not a crash-recovery protocol.
- **On-disk Reverification**: All written bytes are re-read and hashed from disk prior to final
  atomic commit via `fs::rename`.
- **Unix Modes**: Enforces executable mode `0o755` (`rwxr-xr-x`) on binaries and payload mode
  `0o644` (`rw-r--r--`) on image archives.
- **Idempotency**: Repeated materialization preserves file content and permissions.

## Asset selection across variants and scopes (E06.03/#50)

`AssetSelector` resolves delivery modes (`Bundled`, `RegistryPull`, `HostSupplied`, `Disabled`,
or `UnsupportedTarget`) for each asset across:
- **Online vs Offline Variants**: Offline variants bundle all supported optional container images
  (local-path provisioner, local-path helper, default Portainer agent, D2K). Online variants select
  `RegistryPull` for optional images to maintain minimal distribution bundle footprint.
- **External Dependency Builds**: Under `Scope::LegacyExternalDeps`, all 13 supervised executable
  roles are resolved as `HostSupplied` with zero bundled binary payloads, matching the pinned Go
  build tag's absence of embedded executables.
- **Local Storage Toggle**: Disabling local storage (`with_local_storage(false)`) skips extraction
  and deployment of both the local-path provisioner and its helper image.
  Selection applies to archive, directory, provider and single-asset materialization.
  The single-asset API returns `MaterializationError::NotSelected` for a nonbundled
  selection before reading payload bytes or creating the destination/staging directory.
- **Portainer Custom Images & Egress Validation**: Custom Portainer agent image overrides remain
  explicit registry pulls (runtime import delegated to E09). When network egress is denied
  (`with_egress_denied(true)`), any active asset requiring a registry pull is rejected with
  `SelectionError::EgressDeniedRegistryRequired`.
- **Target Policy Enforcement**: Unsupported architecture combinations (e.g. Portainer on `riscv64`,
  D2K on `armv7`/`riscv64`) are enforced per target policy via `SelectionError::UnsupportedTargetFeature`.

## Release artifact packaging (E27.02/#115)

`ReleasePackager` structures, verifies, and validates release packaging across the complete matrix:
- **Node Archives (16 cells)**: Generates and validates metadata (`NodeArchiveArtifact`) across all 16
  cells (cell IDs 1..=16: 4 architectures × 2 libcs × 2 variants). Validates standard archive naming
  format and cell target metadata, with lowercase SHA-256 digest syntax validation. Descriptor checks
  do not inspect archive contents, recorded sizes, or the `bundled_assets` list.
- **Layout & Installation Smoke Checks**: `ReleasePackager::smoke_check_node_archive_layout` tests that
  each cell archive unpacks into an `AssetLayout` via `Materializer`, sets expected executable permissions
  (`0o755`) and payload permissions (`0o644`), and verifies idempotency upon repeated extraction.
- **Cross-Target Management Binaries (4 targets)**: Validates management CLI artifacts (`ManagementArtifact`)
  across Linux/macOS (`linux-amd64`, `linux-arm64`, `darwin-amd64`, `darwin-arm64`), strictly rejecting
  Windows executables (`.exe` or `windows`) per E01.
- **OCI Container Image Manifest Indices**: Binds multi-arch image index descriptors (`OciImageIndexArtifact`)
  across the 4 OCI architectures (`linux/amd64`, `linux/arm64`, `linux/arm/v7`, `linux/riscv64`) with
  format-validated SHA-256 digest strings, supported index media types and consistent unique platform
  descriptors. These metadata checks do not hash registry content. Strictly partitions unsupported platforms (Portainer agent on
  `linux/riscv64`, D2K on `linux/arm/v7` and `linux/riscv64`).
- **Verifiable Release Package Manifest**: `ReleasePackageManifest` provides an exhaustive release ledger
  binding all 16 node archive cells, 4 management binaries, container image index manifests, and explicit
  exclusion declarations. Publication tooling separately hashes actual files and, when supplied with
  per-archive inventories, checks file sizes and materializes each node archive for layout verification.

The publish workflow consumes a prepared `release-candidate` artifact from a successful CI run for
the checked-out default-branch revision. It requires `dist/` containing all 16 canonical node archives
and four management binaries, plus `inventories/<archive-filename>.manifest.json` for every archive.
It fails before publication when the candidate or inventory is absent or invalid. Native CI binaries
alone are not a complete candidate. Current CI uploads only `pr-binaries`, so publication remains
blocked until a trusted producer supplies the complete `release-candidate`. Real candidate assembly, signing, and live release qualification
remain outstanding; this workflow does not claim to implement cryptographic signing.
