#!/bin/sh
# Executed only within a fresh owned guest and a recursively private mount namespace.
set -eu
case_name=$1
bundle=/tmp/rubix-bundle
if grep -Eq ' (shared|master|propagate_from):' /proc/self/mountinfo; then exit 1; fi
case "$case_name" in
 guard|cancel)
  mkdir /tmp/constrained-$case_name-bin
  cp -a /usr/sbin/. /tmp/constrained-$case_name-bin/
  if test "$case_name" = guard; then tool=iptables; double=guard-failure.sh; else tool=modprobe; double=module-wait.sh; fi
  rm -f /tmp/constrained-$case_name-bin/$tool
  cp "$bundle/$double" /tmp/constrained-$case_name-bin/$tool
  chmod 0755 /tmp/constrained-$case_name-bin/$tool
  mount --bind /tmp/constrained-$case_name-bin /usr/sbin
  ;;
 correct|needs_write) ;;
 *) exit 1 ;;
esac
mount --bind /proc/sys /proc/sys
mount -o remount,bind,ro /proc/sys
exec env RUBIX_CONTAINER_FIXTURE_PROTOCOL=1 "$bundle/prepare_node_host" --no-container-mode --disable-ipv6 --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock
