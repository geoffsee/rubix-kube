#!/bin/sh
set -eu
printf 'RUBIX_REBOOT_BEGIN\n'
cat /etc/alpine-release
rc-service cgroups status
test -L /etc/runlevels/boot/cgroups
for controller in cpuset cpu io memory pids; do grep -qw "$controller" /sys/fs/cgroup/cgroup.controllers; done
printf 'CONTROLLERS_reboot '
cat /sys/fs/cgroup/cgroup.controllers
apk info -v | sort > /tmp/packages-reboot
cmp /tmp/packages-installed /tmp/packages-reboot
printf 'PACKAGES_reboot_BEGIN\n'
cat /tmp/packages-reboot
printf 'PACKAGES_reboot_END\n'
RUBIX_ALPINE_GUEST=1 RUBIX_ACTION=cgroups RUBIX_INSTALL=0 timeout 30 /tmp/rubix-bundle/preflight.test -test.run '^TestOwnedGuestPreparation$' -test.timeout 25s -test.v
printf 'RUBIX_REBOOT_COMPLETE\n'
