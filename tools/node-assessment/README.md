# Node assessment qualification

After reviewed source is committed, run `python3 tools/node-assessment/capture.py
--output /tmp/<unique-output>`. It builds pinned Linux arm64 images and runs two
owned, network-disabled, nonroot containers. Only bounded private tmpfs paths are
writable; no host directories or devices are shared. A fixture-only `/usr/sbin`
contains a controlled iptables double; no host command is replaced.

The suite covers nine injected policy cases, eleven actual fixed-process cases,
a configured external-runtime assessment using injected host facts plus the real
probe, and four actual example exits. The external sentinel/config bytes must
survive every command probe. Tests distinguish actual child execution from fake
host capability observations; they do not qualify rule programming or a node.
The ignored tests must never run directly on the development host.

`verify.py <output>` checks exact records, binary/source hashes, reviewed Docker
commands, namespace cleanup and repeated identities. `test_evidence.py` requires
published frozen evidence and tests meaningful mutations, normally and under
`python3 -O`. No success is claimed until those checks pass on final source.

The committed evidence passed twice on source `2f3355235547131d2c5690ff5d22553f79c4a254`. All seven evidence regressions and strict verification pass normally and under `python3 -O`; owned container/image inventories are empty. An earlier attempt stopped before tests because the private fixture mount obscured Docker init through the base image’s `/sbin` symlink. The fixture now separates `/sbin` from `/usr/sbin`; production lookup paths are unchanged.
