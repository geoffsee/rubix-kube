# Agent Guidance

## Project scope and authoritative contracts

Rubix is a Rust workspace developing a single-node Kubernetes distribution with
KubeSolo compatibility. The baseline is Portainer KubeSolo commit
`2ef1c4787989f11f868f81bb84ae2afd4a49a81d`. Read the contracts relevant to a change:

- [Compatibility contract](docs/architecture/compatibility-contract.md): observable
  behavior, deliberate deviations, resource ownership and platform obligations.
- [Component boundary ADR](experiments/component-boundary/ADR.md): selected
  in-process Rust control plane boundary (Option B, amended 2026-10-07),
  Rust responsibilities and experiment limits.
- [Upstream input contract](docs/architecture/upstream-inputs.md) and its
  [inventory](docs/architecture/upstream-inputs.json): authoritative versions,
  source hashes, generation ownership and datastore trust requirements.
- [Acceptance matrix](docs/architecture/acceptance-matrix.md): implementation and
  qualification gates. Crate READMEs describe the implemented slices; a contract
  or passing unit test alone does not establish live cluster qualification.

The production boundary selects an in-process Rust control plane (Option B,
amended 2026-10-07 in [experiments/component-boundary/ADR.md](experiments/component-boundary/ADR.md)),
matching `NodeRuntime` in `crates/rubix-kube/src/runtime.rs`. The control plane
integrates `rubix-datastore` (in-process MVCC/WAL), `rubix-apiserver`, `rubix-controller`,
and addon reconcilers, while `rubix-kubelet` and `rubix-proxy` adapters are
pending integration into default node assembly. Container execution retains
managed containerd (`rubix-containerd`), runtime shims (`containerd-shim-runc-v2`),
OCI runtimes (`crun`), CNI plugins, and workload images. Generated clients do not
implement Kubernetes or a container runtime. Keep retained runtime executables,
shims, CNI programs and addon images explicit in packaging and footprint claims.
Assess any implementation that changes the selected boundary against the ADR
and compatibility contract.

`rubix-kube` implements full configuration parsing, validation, and in-process
`NodeRuntime` assembly for control plane components; worker-node integration
(kubelet CRI execution and kube-proxy host routing) remains under development.
`rubixctl` currently provides management checks, version and help. Do not infer
end-to-end node startup or installation from the presence of component crates.

## Workspace map

| Location | Responsibility |
| --- | --- |
| `crates/rubix-kube` | Node executable and startup adapter; default workspace member |
| `crates/rubixctl` | Separate management executable and explicit prerequisite preparation |
| `crates/rubix-config` | Configuration decoding, precedence, validation and compatibility fixtures |
| `crates/rubix-platform`, `crates/rubix-assets` | Host observations/preflight and dependency inventory/asset verification |
| `crates/rubix-supervisor`, `crates/rubix-pki` | Component lifecycle and certificate/identity management |
| `crates/rubix-apiserver`, `crates/rubix-datastore` | API integration and datastore implementation; consult storage-format contracts before persistence changes |
| `crates/rubix-cri`, `crates/rubix-containerd-api`, `crates/rubix-containerd` | Generated RPC clients and managed runtime integration |
| `crates/rubix-controller`, `crates/rubix-kubelet`, `crates/rubix-proxy` | Kubernetes component adapters |
| `crates/rubix-network`, `crates/rubix-dns`, `crates/rubix-storage`, `crates/rubix-portainer` | Networking and addon behavior |
| `tools/dev` (`rubix-dev`) | Repository maintenance, capture, verification and fixture binaries |
| `tools/upstream` (`rubix-upstream-codegen`) | Explicit upstream client generation; outside the runtime dependency graph |
| `tools/parity`, `tools/integration`, `experiments` | Independent compatibility fixtures and disposable live evidence |
| `third_party/saphyr-parser` | Local crates.io patch; preserve characterized YAML parsing behavior |

## Development and validation

Use the pinned Rust `1.97.1` toolchain, edition 2024 and locked dependency inputs.
Bare Cargo build/test commands select only `rubix-kube`; select a package or use
`--workspace` intentionally. Run focused checks for affected behavior first:

```sh
cargo test --locked -p <affected-package>
cargo clippy --locked -p <affected-package> --all-targets --all-features -- -D warnings
```

The repository CI checks are defined in [.github/workflows/ci.yml](.github/workflows/ci.yml).
For workspace-wide changes, use its validation commands:

```sh
cargo fmt-check
cargo lint
CI=1 cargo nextest run --locked --workspace --all-targets --all-features --profile ci --no-capture
cargo test --locked --workspace --doc --all-features
cargo run --locked -p rubix-dev --bin rubix-language-policy
```

Pull-request CI runs debug tests with cargo-nextest, doctests with `cargo test --doc`, and
`cargo deny --locked --all-features check`. In headless or agent execution environments,
prefix `cargo nextest` with `CI=1` (or export `NEXTEST_HIDE_PROGRESS_BAR=1 NEXTEST_SHOW_PROGRESS=none NEXTEST_NO_INPUT_HANDLER=1`)
and pass `--no-capture` so Nextest suppresses interactive terminal progress bars, pager pagination,
keyboard input listeners, and prevents output capturing deadlocks. Default-branch pushes and merge-group
runs also run release tests through the same nextest profile, release doctests, the
`rubix-kube` and `rubixctl` `--profile dist` build, and upstream input verification. `.cargo/config.toml` defines the
`lint`, `fmt-check`, `test-all` and `doc-all` aliases; `test-all` does not include
the separate doctest pass. Run checks appropriate to the change and report any
unavailable environment or unexecuted qualification instead of implying a pass.

Follow [development tooling policy](docs/architecture/development-tooling.md):
new repository-owned executable tooling and tests use Rust, with reusable logic
in `tools/dev`. Do not add Python scripts, inline Python or Python test suites.
Shell is suitable for short command sequences. Preserve historical evidence and
verification behavior when porting existing tooling; upstream sources and external
tools retain their own implementations.

## Compatibility, generation and evidence rules

- Preserve `kubesolo.io/v1alpha1`, `KUBESOLO_*` inputs, `/etc/kubesolo` and
  `/var/lib/kubesolo` defaults, and established resource identities. Executable
  branding does not authorize renaming persisted schemas or paths. Configuration
  precedence is defaults < file < environment < explicit flags; empty, false,
  zero and absent inputs have distinct semantics.
- Preserve ownership boundaries: external runtimes and unrelated host resources
  are not managed state. Cleanup must identify owned resources; persisted volume
  data, credentials and datastore state require their documented retention rules.
  Consult datastore backup compatibility documentation before assuming an on-disk
  format can replace or adopt a Kine SQLite database.
- Under the amended Option B boundary, `rubix-apiserver` binds in-process to
  `rubix-datastore` via `KubernetesStorage`; external datastore access (when enabled)
  retains a dedicated datastore CA and separate client identity. Preserve this trust
  boundary and test authentication failures; plaintext spike evidence does not
  qualify production transport.
- Follow [upstream tooling instructions](tools/upstream/README.md) for generation.
  Inputs are pinned in `tools/upstream/inputs.json`; preparation is an explicit
  network operation and defaults to `target/upstream`. Ordinary builds and node
  startup must not fetch generation inputs. Regenerate files under
  `crates/rubix-cri/src/generated` and `crates/rubix-containerd-api/src/generated`
  with the verified generator instead of editing them manually. Run the relevant
  `check-cri`, `check-containerd`, `check-kubernetes-bindings` and drift checks
  after changing their inputs or outputs, following CI's preparation sequence.
- Use independent pinned fixtures under `tools/parity` for compatibility changes.
  Preserve raw streams, source inventories, hashes, failures and cleanup receipts;
  do not update expected results simply to agree with the implementation.
- Follow [live integration instructions](tools/integration/README.md) and the
  specific fixture README before running host/runtime tests. Use disposable Linux
  containers or VMs and fresh capture directories. Historical captures establish
  their recorded revision only; current-source claims need fresh evidence.
  macOS unit tests do not qualify Linux runtime, networking or platform behavior.

## Using the installed skills

Engineering and Rust skills documented below live in `.agents/skills/`.
Context7's `find-docs` skill is installed globally, as described below. Use
skills when their guidance addresses a decision in the current task, including
implementation, design, review, and debugging.

1. Select the smallest relevant set. Use a skill explicitly requested by the
   user; otherwise choose by the problem being solved. Routine documentation or
   mechanical edits usually do not need these engineering skills.
2. Briefly state which skill(s) you are applying and why before using them.
3. Read each selected `SKILL.md` before applying it. Read only the supporting
   `references/` files needed for the task, resolving paths relative to that
   skill's directory. The tables below are selection guides; the skill files
   contain the detailed workflows and checklists.
4. Start with the skill that owns the main decision, then add complementary
   skills for implementation or verification. Reassess the selection if the
   task introduces another concern, such as persisted-data changes.
5. Apply the guidance within the user's requested scope and existing repository
   contracts. Prefer the smallest useful change and explain material tradeoffs.
   Skill availability alone is not a reason to introduce a new architecture,
   dependency, abstraction, or unrelated refactoring.
6. Adapt examples to this Rust workspace: use structs, newtypes, enums, traits,
   ownership, `Option`, `Result`, and iterators where appropriate. Prefer a
   skill's Rust examples when available; preserve established behavior and
   Kubernetes or external API compatibility when translating generic advice.
7. Verify the affected behavior with appropriate checks. Use existing coverage
   where sufficient, add meaningful tests for changed behavior or risk, and
   report what was checked and any remaining limitations.

## Current documentation with Context7

Context7 is available through the global `find-docs` skill (for example in
`~/.agents/skills/find-docs/SKILL.md` or global agent configuration). Read the
skill when applying this workflow. Use the global installation directly; a
project-local copy or global installation of the `ctx7` executable is not
required because commands use `npx ctx7@latest`.

Use Context7 whenever the user asks about a library, framework, SDK, API, CLI
tool, or cloud service. This includes Rust crates, API syntax, configuration,
setup, version migration, CLI usage, and library-specific debugging, even when
the API seems familiar. Prefer it over web search for library documentation
and verify the answer against retrieved documentation.

Do not use it for standalone refactoring, scripts written from scratch,
business-logic debugging, code review, or general programming concepts.

### Lookup workflow

1. Resolve the official library name, including punctuation, with a specific
   query describing the documentation needed:

   ```bash
   npx ctx7@latest library "<official library name>" "<specific documentation question>"
   ```

2. Choose the best result by exact name match, description relevance, snippet
   coverage, source reputation (prefer High or Medium), and benchmark score.
   Use an alternate spelling or refined query if the results are unsuitable.
   Always resolve first unless the user supplies a valid `/org/project` or
   `/org/project/version` ID; do not invent an ID from the package name.
3. Fetch documentation using the returned ID:

   ```bash
   npx ctx7@latest docs "<returned library ID>" "<specific documentation question>"
   ```

4. Base implementation advice on the fetched documentation and its source
   links. Match the version in the relevant Cargo manifest and lockfile; use a
   `/org/project/version` ID from the resolver output when available. If the
   requested version is unavailable, state that limitation instead of treating
   another version's API as verified for this workspace.

Keep each query focused on one concept, unless the question concerns how
concepts interact. Use separate `docs` calls for distinct concepts while
staying within **three Context7 commands total per user question**, including
resolution, refinements, and retries. A normal lookup uses one `library` call
and one `docs` call, leaving one call for refinement or a second concept.
Queries must exclude credentials, personal data, and proprietary source code.

Run Context7 requests outside the default sandbox, within the session's
execution policy. This workspace already has sandboxing disabled. On DNS or
network failures such as `ENOTFOUND` or `fetch failed`, ensure the request runs
outside the sandbox rather than repeating it inside. On quota errors, inform
the user and suggest `npx ctx7@latest login` or setting `CONTEXT7_API_KEY` for
higher limits. Never print the key or include it in a query, and never silently
substitute training knowledge for a failed documentation lookup.

For Rust crate and library API documentation, this Context7 workflow takes
precedence over the upstream Rust skills' browser/research tool preferences.
Use `rust-router` and the relevant engineering skills to interpret the results
and apply them to the repository's design and compatibility requirements.

## Rust skills installation and project settings

Rust skills come from [actionbook/rust-skills](https://github.com/actionbook/rust-skills).
The repository is installed as the `.rust-skills` Git submodule following
[the Codex installation guide](.rust-skills/.codex/INSTALL.md). All 38 skills
listed in its plugin manifest are copied into `.agents/skills/` for discovery.
The submodule pins the source revision; refresh the discovery copies from the
same revision after an intentional upstream update.

See [.rust-skills/AGENTS.md](.rust-skills/AGENTS.md) for Rust development
guidelines. This project's instructions and configuration take precedence over
upstream defaults:

- Keep edition 2024 and the workspace's `rust-version = "1.97"` from
  [Cargo.toml](Cargo.toml). Follow [rust-toolchain.toml](rust-toolchain.toml).
- Preserve the workspace's `unsafe_code = "deny"`, Clippy `all = "deny"`, and
  `await_holding_lock = "deny"` policies. Upstream warning-level examples do not
  authorize weakening these settings or introducing unsafe code.
- Follow [rustfmt.toml](rustfmt.toml), including its 100-character width, and
  existing naming conventions. Propagate recoverable library failures through
  `Result` and `?` with useful context.
- Use installed skills through their `SKILL.md` files. This Codex integration
  does not install Claude plugin hooks, slash-command handlers, LSP tools, or
  browser tooling. Use available tools and documented inline fallbacks; use
  `rg` and source inspection when semantic navigation is unavailable, and
  distinguish text search from a complete semantic reference search.

## Routing Rust work

Start Rust coding, design, compiler-error, and Cargo questions with
[rust-router](.agents/skills/rust-router/SKILL.md), then load the relevant
language or design skill and domain context. Diagnose ownership and design
requirements before applying a local compiler fix such as cloning or adding
interior mutability.

For this workspace, Kubernetes, containers, gRPC, and service lifecycle work
use `domain-cloud-native` with `m07-concurrency`; HTTP handlers use `domain-web`
with `m07-concurrency`; CLI behavior uses `domain-cli` with `m07-concurrency`.
Add the specific ownership, error, or other skill indicated by the problem.
Keep domain and protocol compatibility requirements in view throughout.

### Core routing, style, and research

| Skill | When and how to use it |
| --- | --- |
| [rust-router](.agents/skills/rust-router/SKILL.md) | Route Rust questions by compiler error, language mechanism, design concern, and domain; select the relevant skills before implementing a fix. |
| [coding-guidelines](.agents/skills/coding-guidelines/SKILL.md) | Review Rust naming, conversions, documentation, formatting, and idioms against this workspace's configuration. Treat generic dependency suggestions as choices requiring justification. |
| [rust-learner](.agents/skills/rust-learner/SKILL.md) | Research Rust releases, crate versions, features, or API documentation. Use Context7 first for library/API documentation as described above; verify release and version information against primary sources and the project's pinned versions. |
| [unsafe-checker](.agents/skills/unsafe-checker/SKILL.md) | Review unsafe or FFI boundaries for lifetimes, alignment, aliasing, ABI, and safety documentation. Prefer safe APIs and preserve the workspace's unsafe-code policy. |

### Language mechanics and design

| Skill | When and how to use it |
| --- | --- |
| [m01-ownership](.agents/skills/m01-ownership/SKILL.md) | Resolve moves, borrows, and lifetimes, including E0382 and E0597, by identifying who owns data and how long it must live. |
| [m02-resource](.agents/skills/m02-resource/SKILL.md) | Choose `Box`, `Rc`, `Arc`, `Weak`, or interior-mutability wrappers according to ownership, sharing, and thread-safety requirements. |
| [m03-mutability](.agents/skills/m03-mutability/SKILL.md) | Resolve E0596, E0499, and E0502; make mutation scope and synchronization explicit before adding `RefCell` or locks. |
| [m04-zero-cost](.agents/skills/m04-zero-cost/SKILL.md) | Design generics and trait bounds or choose static versus dynamic dispatch; diagnose type and trait errors using the intended interface. |
| [m05-type-driven](.agents/skills/m05-type-driven/SKILL.md) | Use enums, newtypes, typestate, and validated construction when types can prevent meaningful invalid states. |
| [m06-error-handling](.agents/skills/m06-error-handling/SKILL.md) | Choose `Option`, `Result`, error types, context, and propagation; distinguish expected failure from a programming bug. |
| [m07-concurrency](.agents/skills/m07-concurrency/SKILL.md) | Design async tasks, threads, channels, cancellation, and synchronization; check `Send`/`Sync`, lock scope, races, and blocking work. |
| [m09-domain](.agents/skills/m09-domain/SKILL.md) | Translate domain roles, invariants, entities, and value objects into Rust types; pair with `ddd-best-practices` for ownership and aggregate boundaries. |
| [m10-performance](.agents/skills/m10-performance/SKILL.md) | Investigate measured bottlenecks with profiling and representative benchmarks before changing allocation, data layout, or algorithms. |
| [m11-ecosystem](.agents/skills/m11-ecosystem/SKILL.md) | Integrate crates, Cargo features, workspace dependencies, or bindings; check compatibility and actual API requirements before adding dependencies. |
| [m12-lifecycle](.agents/skills/m12-lifecycle/SKILL.md) | Define initialization, pooling, guards, `Drop`, cleanup, and shutdown behavior, including error and cancellation paths. |
| [m13-domain-error](.agents/skills/m13-domain-error/SKILL.md) | Design error categories, retry/backoff, fallback, and recovery ownership; distinguish transient failures from permanent domain rejection. |
| [m14-mental-model](.agents/skills/m14-mental-model/SKILL.md) | Explain Rust concepts or correct misconceptions with concrete ownership, memory, and lifecycle examples. |
| [m15-anti-pattern](.agents/skills/m15-anti-pattern/SKILL.md) | Review unnecessary cloning, panic-prone production code, and other Rust design smells; address the underlying requirement rather than applying blanket replacements. |

### Domain context

| Skill | When and how to use it |
| --- | --- |
| [domain-cloud-native](.agents/skills/domain-cloud-native/SKILL.md) | Apply Kubernetes, container, gRPC, observability, health-check, and graceful-shutdown constraints to service design. |
| [domain-web](.agents/skills/domain-web/SKILL.md) | Design Rust HTTP services, middleware, handlers, and shared state; combine with `rest-api-best-practices` for the public API contract. |
| [domain-cli](.agents/skills/domain-cli/SKILL.md) | Design arguments, configuration precedence, terminal output, exit codes, and completion for CLI tools such as `rubixctl`. |
| [domain-embedded](.agents/skills/domain-embedded/SKILL.md) | Apply memory, allocation, interrupt, and real-time constraints when working on embedded or `no_std` targets. |
| [domain-iot](.agents/skills/domain-iot/SKILL.md) | Design device, telemetry, MQTT, or edge workflows around connectivity, resource, and security constraints when such work is in scope. |
| [domain-fintech](.agents/skills/domain-fintech/SKILL.md) | Apply precision, auditability, and consistency requirements when financial or ledger behavior is explicitly involved. |
| [domain-ml](.agents/skills/domain-ml/SKILL.md) | Address tensor, inference, training, or accelerator integration constraints when implementing Rust ML functionality. |

### Navigation and specialized workflows

| Skill | When and how to use it |
| --- | --- |
| [rust-code-navigator](.agents/skills/rust-code-navigator/SKILL.md) | Locate definitions, references, and type information using LSP when available; otherwise inspect source with the limitations described above. |
| [rust-symbol-analyzer](.agents/skills/rust-symbol-analyzer/SKILL.md) | Inspect modules, structs, traits, and functions to understand project or file structure. |
| [rust-trait-explorer](.agents/skills/rust-trait-explorer/SKILL.md) | Find trait implementations and inspect how types satisfy interface contracts. |
| [rust-call-graph](.agents/skills/rust-call-graph/SKILL.md) | Trace callers and callees to understand execution paths and change impact; prefer semantic call hierarchy when available. |
| [rust-deps-visualizer](.agents/skills/rust-deps-visualizer/SKILL.md) | Explain workspace or crate dependency graphs using Cargo metadata/tree and the relevant feature selection. |
| [rust-refactor-helper](.agents/skills/rust-refactor-helper/SKILL.md) | Plan Rust renames, moves, and extractions using reference analysis; pair with `refactoring-best-practices` and verify affected callers. |
| [rust-daily](.agents/skills/rust-daily/SKILL.md) | Produce requested Rust news or periodic reports from fresh sources, stating the covered dates. |
| [rust-skill-creator](.agents/skills/rust-skill-creator/SKILL.md) | Create a Rust crate or standard-library skill when requested, based on verified documentation and the relevant version. |
| [meta-cognition-parallel](.agents/skills/meta-cognition-parallel/SKILL.md) | Use for an explicitly requested three-layer analysis, such as `/meta-parallel`; follow the skill's agent or inline mode according to available capabilities. |
| [core-actionbook](.agents/skills/core-actionbook/SKILL.md) | Internal support only when a selected research workflow explicitly needs Actionbook selectors and that MCP tool is available. |
| [core-agent-browser](.agents/skills/core-agent-browser/SKILL.md) | Internal support only when a selected workflow explicitly requires browser automation and the CLI is available. |
| [core-dynamic-skills](.agents/skills/core-dynamic-skills/SKILL.md) | Manage generated crate skills only when explicitly requested through sync, update, or clean workflows; check destination paths for this Codex setup. |
| [core-fix-skill-docs](.agents/skills/core-fix-skill-docs/SKILL.md) | Check or repair generated skill documentation only when explicitly requested; scope changes to the requested crate or documentation. |

## Engineering best-practice skills

| Skill | Use when | How to apply it |
| --- | --- | --- |
| [data-migration-best-practices](.agents/skills/data-migration-best-practices/SKILL.md) | Moving or transforming persisted data, changing schemas, backfilling, or planning cutover and recovery. | Define source authority and invariants; use compatible expansion, bounded idempotent batches, committed checkpoints, and live-change capture when needed. Reconcile values and identities and gate cutover on correctness and freshness. Plan rollback or forward repair before contraction. |
| [ddd-best-practices](.agents/skills/ddd-best-practices/SKILL.md) | Deciding domain ownership, bounded contexts, aggregate invariants, domain events, typed domain failures, or repository contracts. | Start from domain language and commands. Define atomic consistency boundaries, keep aggregates small, and assign rules to their owners. Separate domain meaning from application orchestration and infrastructure mechanics. |
| [design-patterns-best-practices](.agents/skills/design-patterns-best-practices/SKILL.md) | Choosing a pattern for behavior variation, construction, state transitions, external adapters, query criteria, or layered responsibilities. | Identify the concrete design pressure, compare the lightest suitable patterns, and justify how the choice reduces coupling or makes change local. Prefer a simpler extraction when it solves the problem. |
| [fp-best-practices](.agents/skills/fp-best-practices/SKILL.md) | Designing transformations, composing functions, separating side effects, modeling absence/failure, or reviewing shared mutable state. | Keep deterministic logic separate from I/O. Use explicit inputs, small composable transformations, enums, `Option` and `Result`; favor clear iterator pipelines and explicit effect boundaries. |
| [infrastructure-design](.agents/skills/infrastructure-design/SKILL.md) | Designing transactions, optimistic concurrency, reliable event delivery, Outbox/Inbox, retries, CDC, caches, or database views. | Establish consistency and delivery requirements first. Put technical adapters behind core contracts; define transaction scope, durable handoff, idempotency, ordering, and failure handling. Add caches for measured needs with explicit freshness and invalidation policies. |
| [oop-best-practices](.agents/skills/oop-best-practices/SKILL.md) | Improving type and method responsibilities, encapsulation, value objects, collection invariants, naming, or dependency roles. | Keep behavior with the concept that owns its rules. Use cohesive structs and small trait contracts, composition, and validated newtypes where semantics justify them. Preserve consistent equality and hashing and restrict mutation that bypasses invariants. |
| [refactoring-best-practices](.agents/skills/refactoring-best-practices/SKILL.md) | Restructuring existing code while preserving behavior, isolating hard dependencies, or incrementally evolving error and persistence contracts. | Establish the observable behavior baseline, add characterization coverage when needed, and create narrow seams. Make small reversible changes and rerun relevant checks after each meaningful step, preserving outputs, side effects, and failure semantics. |
| [rest-api-best-practices](.agents/skills/rest-api-best-practices/SKILL.md) | Designing or reviewing HTTP endpoints, methods, status codes, error bodies, pagination, filtering, versioning, or API security. | Start from consumer capabilities and existing protocol contracts. Choose intentional HTTP semantics and consistent response/error shapes, bound collections, and protect sensitive details. Preserve required Kubernetes and upstream wire formats rather than replacing them with generic conventions. |
| [tdd-best-practices](.agents/skills/tdd-best-practices/SKILL.md) | Driving behavior through tests, choosing test scope or doubles, verifying invariants and I/O boundaries, or fixing brittle tests. | Follow Red-Green-Refactor for test-driven implementation. Test public behavior and failure paths with deterministic fixtures. Use doubles at I/O boundaries and real disposable infrastructure when transaction, concurrency, persistence, or protocol behavior is under test. |

## Combining skills

- **Domain and type design:** start with `ddd-best-practices` for ownership and
  consistency boundaries; use `oop-best-practices` for encapsulation or
  `fp-best-practices` for transformations and typed composition. Add
  `m09-domain` and `m05-type-driven` for Rust modeling, and
  `tdd-best-practices` for invariant and failure-contract tests.
- **Safe code cleanup:** start with `refactoring-best-practices`, then use OOP,
  FP, or design-pattern guidance for the specific change. Use characterization
  tests to protect behavior that existing tests do not cover. Use
  `rust-refactor-helper` for Rust reference and rename analysis.
- **HTTP changes:** use `rest-api-best-practices` for the public contract, DDD
  when domain semantics are involved, and TDD for request/response and failure
  coverage. Add `domain-web`, `m07-concurrency`, and `m06-error-handling` for
  Rust handlers and failure propagation.
- **Persistence and event delivery:** use DDD for business consistency rules,
  `infrastructure-design` for transactions and delivery mechanics, and TDD for
  rollback, concurrency, duplicate, and retry verification.
- **Persisted-data migration:** lead with `data-migration-best-practices`; add
  infrastructure guidance for capture and transactions and TDD for restart,
  reconciliation, and recovery tests. Refactoring guidance covers supporting
  code seams and compatibility paths; it does not replace migration planning.

Distinguish overlapping concerns: DDD owns aggregate and repository meaning;
infrastructure owns persistence and delivery mechanics. OOP and FP guide code
structure; design-pattern guidance helps choose a collaboration pattern.
Refactoring preserves existing behavior; migration changes persisted data and
requires its own correctness and recovery plan.
