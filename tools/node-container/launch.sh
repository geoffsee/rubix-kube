#!/bin/sh
# Owned disposable VM only. No PID namespace, fork option, or background helper.
set -eu
root=$1
printf 'LAUNCH_PID %s\n' "$$"
printf '%s' "$$" > "$root/cgroup.procs"
exec /usr/bin/unshare --mount --cgroup --propagation private /bin/sh /tmp/rubix-bundle/namespace.sh "$root"
