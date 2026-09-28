#!/bin/sh
# This script is only sent to the owned disposable Alpine VM after artifact verification.
set -eu
ulimit -f 2048
bundle=/tmp/rubix-bundle
binary=$bundle/prepare_host_network
printf 'SETUP_BEGIN\n'
uname -a
cat /etc/alpine-release
printf '/tmp/rubix-bundle/repo\n' > /etc/apk/repositories
apk add --no-network nftables iptables
rc-service cgroups start
for controller in cpuset cpu io memory pids; do grep -qw "$controller" /sys/fs/cgroup/cgroup.controllers; done
# Fresh-guard prerequisites are explicit fixture setup, not preparation-under-test.
printf '1' > /proc/sys/net/ipv4/ip_forward
modprobe xt_comment
mkdir -p /tmp/external-runtime
printf 'host-owned configuration sentinel\n' > /tmp/external-runtime/config.toml
printf 'host-owned state sentinel\n' > /tmp/external-runtime/state
sha256sum /tmp/external-runtime/* > /tmp/external-runtime-before
printf 'SETUP_END\n'
controls='all default lo'
zero() { for item in $controls; do printf '0' > /proc/sys/net/ipv6/conf/$item/disable_ipv6; done; }
scalars() {
 printf 'SCALARS_%s ' "$1"
 for item in $controls; do value=$(cat /proc/sys/net/ipv6/conf/$item/disable_ipv6); printf '%s=%s ' "$item" "$value"; done
 printf '\n'
}
run() {
 name=$1; expected=$2; shift 2
 set +e
 timeout 180 "$binary" "$@" > /tmp/network-out 2> /tmp/network-err
 code=$?
 set -e
 frame "$name" "$code"
 test "$code" = "$expected"
}
frame() {
 printf 'CASE_%s_BEGIN\nEXIT %s\nSTDOUT_BEGIN\n' "$1" "$2"
 cat /tmp/network-out
 printf 'STDOUT_END\nSTDERR_BEGIN\n'
 cat /tmp/network-err
 printf 'STDERR_END\nCASE_%s_END\n' "$1"
}
identity() {
 phase=$1; name=$2
 path=/usr/sbin/$name
 if test -L "$path"; then
  printf '%s_KIND_%s LINK %s\n' "$phase" "$name" "$(readlink "$path")"
 elif test -f "$path"; then
  printf '%s_KIND_%s REGULAR\n' "$phase" "$name"
 else
  printf '%s_KIND_%s ABSENT\n' "$phase" "$name"
 fi
 resolved=$(PATH=/usr/sbin:/usr/bin:/sbin:/bin command -v "$name")
 printf '%s_RESOLVED_%s ' "$phase" "$name"; sha256sum "$resolved"
}
backup() {
 name=$1
 identity ORIGINAL "$name"
 if test -e /usr/sbin/$name || test -L /usr/sbin/$name; then
  cp -P /usr/sbin/$name /tmp/network-original-$name
  printf 'ORIGINAL_%s ' "$name"; sha256sum /usr/sbin/$name
 else
  printf 'ORIGINAL_%s ABSENT\n' "$name"
 fi
}
restore() {
 name=$1
 rm /usr/sbin/$name
 if test -e /tmp/network-original-$name || test -L /tmp/network-original-$name; then
  cp -P /tmp/network-original-$name /usr/sbin/$name
  printf 'RESTORED_%s ' "$name"; sha256sum /usr/sbin/$name
 else
  printf 'RESTORED_%s ABSENT\n' "$name"
 fi
 identity RESTORED "$name"
}
backup modprobe
backup iptables
rm -f /usr/sbin/modprobe /usr/sbin/iptables
cp "$bundle/modprobe-double.sh" /usr/sbin/modprobe
cp "$bundle/iptables-double.sh" /usr/sbin/iptables
chmod 0755 /usr/sbin/modprobe /usr/sbin/iptables
printf 'DOUBLE_modprobe '; sha256sum /usr/sbin/modprobe
printf 'DOUBLE_iptables '; sha256sum /usr/sbin/iptables
printf 'fail\n' > /tmp/rubix-network-mode
zero
scalars initial
run help 0 --help
run version 0 --version
run print 0 --print-config
test ! -e /tmp/rubix-network-attempts
run guard_failed 1 --disable-ipv6 --container-mode=false --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock
test ! -e /tmp/rubix-network-attempts
scalars guard_failed
restore iptables
# Failed module doubles settle and continue, while IPv6 operations remain real.
run double_failed 0 --disable-ipv6 --container-mode=false --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock
printf 'ATTEMPTS_double_failed_BEGIN\n'; cat /tmp/rubix-network-attempts; printf 'ATTEMPTS_double_failed_END\n'
scalars double_failed
rm /tmp/rubix-network-attempts
zero
printf 'limits\n' > /tmp/rubix-network-mode
run double_limits 0 --disable-ipv6 --container-mode=false --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock
printf 'ATTEMPTS_double_limits_BEGIN\n'; cat /tmp/rubix-network-attempts; printf 'ATTEMPTS_double_limits_END\n'
scalars double_limits
rm /tmp/rubix-network-attempts
zero
printf 'wait\n' > /tmp/rubix-network-mode
PRIVATE_SENTINEL=must-not-reach-modprobe "$binary" --disable-ipv6 --container-mode=false --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock > /tmp/network-out 2> /tmp/network-err &
consumer=$!
tries=0
until test -f /tmp/rubix-network-waiting; do
 tries=$((tries+1)); test "$tries" -lt 100
 sleep 0.02
done
kill -INT "$consumer"
set +e
wait "$consumer"
code=$?
set -e
frame double_cancel "$code"
test "$code" = 1
printf 'ATTEMPTS_double_cancel_BEGIN\n'; cat /tmp/rubix-network-attempts; printf 'ATTEMPTS_double_cancel_END\n'
scalars double_cancel
restore modprobe
# Genuine execution begins only after the original command bytes/links are restored.
printf 'MODULES_BEFORE_BEGIN\n'; cat /proc/modules; printf 'MODULES_BEFORE_END\n'
zero
run real_first 0 --disable-ipv6 --container-mode=false --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock
scalars real_first
run real_repeat 0 --disable-ipv6 --container-mode=false --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock
scalars real_repeat
printf 'MODULES_AFTER_BEGIN\n'; cat /proc/modules; printf 'MODULES_AFTER_END\n'
sha256sum /tmp/external-runtime/* > /tmp/external-runtime-after
cmp /tmp/external-runtime-before /tmp/external-runtime-after
printf 'EXTERNAL_SENTINEL_UNCHANGED\n'
printf 'NETWORK_GUEST_COMPLETE\n'
