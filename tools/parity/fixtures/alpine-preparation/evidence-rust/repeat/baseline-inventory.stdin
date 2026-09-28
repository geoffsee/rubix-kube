#!/bin/sh
set -eu
printf 'RUBIX_ALPINE_BOOT_BEGIN\n'
cat /etc/alpine-release
uname -a
id
readlink /proc/1/exe
apk --version
rc-status -a
inventory() {
 printf 'PACKAGES_%s_BEGIN\n' "$1"
 apk info -v | sort
 printf 'PACKAGES_%s_END\n' "$1"
}
controllers() {
 if test -f /sys/fs/cgroup/cgroup.controllers; then
  printf 'CONTROLLERS_%s ' "$1"
  cat /sys/fs/cgroup/cgroup.controllers
 else printf 'CONTROLLERS_%s ABSENT\n' "$1"; fi
}
if test "${RUBIX_RUN_PREPARATION:-0}" = 1; then
 printf 'REPOSITORIES_BEFORE_BEGIN\n'
 cat /etc/apk/repositories
 printf 'REPOSITORIES_BEFORE_END\n'
 printf '/tmp/rubix-bundle/repo\n' > /etc/apk/repositories
fi
inventory before
controllers before
printf 'RUBIX_ALPINE_BOOT_COMPLETE\n'
if test "${RUBIX_RUN_PREPARATION:-0}" != 1; then exit 0; fi
# These preconditions distinguish actual missing-prerequisite recovery from no-op success.
test ! -f /sys/fs/cgroup/cgroup.controllers
for tool in /usr/sbin/nft /sbin/nft /usr/bin/nft /sbin/iptables /usr/sbin/iptables /bin/iptables /usr/bin/iptables; do test ! -e "$tool"; done
oracle() { RUBIX_ALPINE_GUEST=1 RUBIX_ACTION="$1" RUBIX_INSTALL="$2" timeout 30 /tmp/rubix-bundle/preflight.test -test.run '^TestOwnedGuestPreparation$' -test.timeout 25s -test.v; }
apk info -v | sort > /tmp/packages-before
oracle network 0
oracle cgroups 0
apk info -v | sort > /tmp/packages-denied
cmp /tmp/packages-before /tmp/packages-denied
test ! -e /etc/runlevels/boot/cgroups
test ! -f /sys/fs/cgroup/cgroup.controllers
printf 'RUBIX_NO_OPT_IN_UNCHANGED\n'
oracle network 1
inventory installed
nft --version
iptables --version
apk info -v | sort > /tmp/packages-installed
oracle network 1
apk info -v | sort > /tmp/packages-repeated
cmp /tmp/packages-installed /tmp/packages-repeated
printf 'RUBIX_PACKAGE_REPEAT_UNCHANGED\n'
oracle cgroups 1
test -L /etc/runlevels/boot/cgroups
controllers after
for controller in cpuset cpu io memory pids; do grep -qw "$controller" /sys/fs/cgroup/cgroup.controllers; done
rc-service cgroups status
rc-update show boot
oracle cgroups 1
controllers repeated
printf 'RUBIX_PREPARATION_COMPLETE\n'
