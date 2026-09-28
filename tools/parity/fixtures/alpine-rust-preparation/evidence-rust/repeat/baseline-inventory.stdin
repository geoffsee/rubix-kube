#!/bin/sh
set -eu
printf '/tmp/rubix-bundle/repo\n' > /etc/apk/repositories
inventory() { printf 'PACKAGES_%s_BEGIN\n' "$1"; apk info -v | sort; printf 'PACKAGES_%s_END\n' "$1"; }
controllers() { printf 'CONTROLLERS_%s ' "$1"; if test -f /sys/fs/cgroup/cgroup.controllers; then cat /sys/fs/cgroup/cgroup.controllers; else printf 'ABSENT\n'; fi; }
run() {
 name="$1"; expected="$2"; shift 2
 set +e
 timeout 40 /tmp/rubix-bundle/rubixctl check "$@" > /tmp/rubix-out 2> /tmp/rubix-err
 code="$?"
 set -e
 printf 'CASE_%s_BEGIN\nEXIT %s\nSTDOUT_BEGIN\n' "$name" "$code"
 cat /tmp/rubix-out
 printf 'STDOUT_END\nSTDERR_BEGIN\n'
 cat /tmp/rubix-err
 printf 'STDERR_END\nCASE_%s_END\n' "$name"
 test "$code" = "$expected"
}
uname -a
cat /etc/alpine-release
inventory before
controllers before
test ! -f /sys/fs/cgroup/cgroup.controllers
test ! -e /etc/runlevels/boot/cgroups
apk info -v | sort > /tmp/packages-before
run opt_out 1
apk info -v | sort > /tmp/packages-denied
cmp /tmp/packages-before /tmp/packages-denied
test ! -e /etc/runlevels/boot/cgroups
test ! -f /sys/fs/cgroup/cgroup.controllers
printf 'NO_OPT_IN_UNCHANGED\n'
# Actual APK failure using an empty guest-local repository, without fake executables.
mkdir /tmp/empty-repo
printf '/tmp/empty-repo\n' > /etc/apk/repositories
run package_failure 1 --install-prereqs
printf '/tmp/rubix-bundle/repo\n' > /etc/apk/repositories
apk info -v | sort > /tmp/packages-failed
cmp /tmp/packages-before /tmp/packages-failed
test ! -e /etc/runlevels/boot/cgroups
printf 'PACKAGE_FAILURE_UNCHANGED\n'
# Actual rc-update registration failure; package action completes first.
printf "SERVICE_ORIGINAL "
sha256sum /etc/init.d/cgroups
mv /etc/init.d/cgroups /tmp/rubix-cgroups-service
run service_failure 1 --install-prereqs
mv /tmp/rubix-cgroups-service /etc/init.d/cgroups
test ! -e /etc/runlevels/boot/cgroups
test ! -f /sys/fs/cgroup/cgroup.controllers
inventory after_service_failure
# Deliberate no-effect service script; actual OpenRC executables remain unchanged.
cp /etc/init.d/cgroups /tmp/rubix-original-service
cp /tmp/rubix-bundle/service-double.sh /etc/init.d/cgroups
chmod 0755 /etc/init.d/cgroups
printf 'SERVICE_DOUBLE '
sha256sum /etc/init.d/cgroups
run injected_no_effect_service 1 --install-prereqs
test ! -f /sys/fs/cgroup/cgroup.controllers
rc-service cgroups zap
cp /tmp/rubix-original-service /etc/init.d/cgroups
cmp /tmp/rubix-original-service /etc/init.d/cgroups
printf 'SERVICE_RESTORED '
sha256sum /etc/init.d/cgroups
run prepare 0 --install-prereqs
inventory installed
controllers after
test -L /etc/runlevels/boot/cgroups
for controller in cpuset cpu io memory pids; do grep -qw "$controller" /sys/fs/cgroup/cgroup.controllers; done
apk info -v | sort > /tmp/packages-installed
run repeat 0 --install-prereqs
apk info -v | sort > /tmp/packages-repeated
cmp /tmp/packages-installed /tmp/packages-repeated
controllers repeated
rc-service cgroups status
printf 'RUST_PREPARATION_COMPLETE\n'
