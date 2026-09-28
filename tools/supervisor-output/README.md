# Bounded merged-output qualification

After committing reviewed source, run `python3 tools/supervisor-output/capture.py
--output /tmp/<unique-output>` from this worktree. It builds pinned Linux arm64
images and runs 13 adverse process cases twice in separate owned, nonroot,
network-disabled containers with no host bind mounts. The escaped-writer case is
finite and deliberately shows incomplete capture; namespace inventory requires it
to have exited before qualification ends. No raw captured child bytes are logged.

`verify.py <output>` verifies exact cases, byte budgets, terminal owner facts,
namespace cleanup, binary/raw hashes and current relevant source inventory.
Cargo.lock, root build configuration, all supervisor source/tests and executable fixture files are
bound; other workspace sources remain in the historical full build inventory.
`python3 -m unittest discover -s tools/supervisor-output` requires frozen evidence. Use `python3 -O` to confirm checks do not depend on assert.
The isolated cases are not permission to run ignored tests on the host. Successful qualification requires both reviewed-source runs and the frozen-evidence
test; this document alone does not assert that qualification has passed.
