#!/bin/sh
set -eu
root=$1
# util-linux --propagation private uses recursive propagation. Capture the complete
# pre-bind table so the outside verifier can prove this happened before remapping.
printf 'PRIVATE_BEFORE_BIND_BEGIN\n'
cat /proc/self/mountinfo
printf 'PRIVATE_BEFORE_BIND_END\n'
if grep -Eq ' (shared|master|propagate_from):' /proc/self/mountinfo; then exit 1; fi
mount --bind "$root" /sys/fs/cgroup
exec env RUBIX_CONTAINER_FIXTURE_PROTOCOL=1 /tmp/rubix-bundle/prepare_node_host --container-mode --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock
