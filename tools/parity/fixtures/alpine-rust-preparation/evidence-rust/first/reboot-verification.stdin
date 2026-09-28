#!/bin/sh
set -eu
test -L /etc/runlevels/boot/cgroups
printf 'CONTROLLERS_reboot '
cat /sys/fs/cgroup/cgroup.controllers
for controller in cpuset cpu io memory pids; do grep -qw "$controller" /sys/fs/cgroup/cgroup.controllers; done
apk info -v | sort > /tmp/packages-reboot
cmp /tmp/packages-installed /tmp/packages-reboot
printf 'PACKAGES_reboot_BEGIN\n'
cat /tmp/packages-reboot
printf 'PACKAGES_reboot_END\n'
set +e
timeout 40 /tmp/rubix-bundle/rubixctl check > /tmp/rubix-out 2> /tmp/rubix-err
code="$?"
set -e
printf 'CASE_reboot_BEGIN\nEXIT %s\nSTDOUT_BEGIN\n' "$code"
cat /tmp/rubix-out
printf 'STDOUT_END\nSTDERR_BEGIN\n'
cat /tmp/rubix-err
printf 'STDERR_END\nCASE_reboot_END\n'
test "$code" = 0
printf 'RUST_REBOOT_COMPLETE\n'
