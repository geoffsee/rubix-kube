#!/bin/sh
# Only in the owned disposable VM after strict source/artifact/input verification.
set -eu
ulimit -f 2048
bundle=/tmp/rubix-bundle
binary=$bundle/prepare_node_host
printf 'SETUP_BEGIN\n'
uname -a
cat /etc/alpine-release
printf '/tmp/rubix-bundle/repo\n' > /etc/apk/repositories
apk add --no-network nftables iptables util-linux-misc=2.42.3-r1
rc-service cgroups start
printf '1' > /proc/sys/net/ipv4/ip_forward
modprobe xt_comment
/usr/bin/unshare --version
printf 'UNSHARE_SHA256 '; sha256sum /usr/bin/unshare
available=$(cat /sys/fs/cgroup/cgroup.controllers)
for controller in cpuset cpu io memory pids; do printf '%s\n' "$available" | grep -qw "$controller"; done
for controller in $available; do printf '+%s' "$controller" > /sys/fs/cgroup/cgroup.subtree_control; done
mkdir /sys/fs/cgroup/rubix-container-real /sys/fs/cgroup/rubix-container-signal /sys/fs/cgroup/rubix-container-sibling
printf '17' > /sys/fs/cgroup/rubix-container-sibling/pids.max
mkdir /tmp/external-runtime
printf 'host-owned configuration sentinel\n' > /tmp/external-runtime/config.toml
printf 'host-owned state sentinel\n' > /tmp/external-runtime/state
printf 'SETUP_END\n'
frame() {
 printf '%s_BEGIN\n' "$1"; cat "$2"; printf '%s_END\n' "$1"
}
run_cli() {
 name=$1; shift
 set +e
 "$binary" "$@" >/tmp/container-cli.out 2>/tmp/container-cli.err
 code=$?
 set -e
 printf 'CLI_%s_BEGIN\nEXIT %s\nSTDOUT_BEGIN\n' "$name" "$code"
 cat /tmp/container-cli.out
 printf 'STDOUT_END\nSTDERR_BEGIN\n'; cat /tmp/container-cli.err
 printf 'STDERR_END\nCLI_%s_END\n' "$name"
 test "$code" = 0
}
run_cli help --help
run_cli version --version
run_cli print --container-mode --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock --print-config
printf 'INJECTED_TESTS_BEGIN\n'
for test in rubix_kube host_preparation host_network; do
 printf 'TEST_%s_BEGIN\n' "$test"
 "$bundle/$test"
 printf 'TEST_%s_END\n' "$test"
done
printf 'INJECTED_TESTS_END\n'
for mode in eof wrong; do
 set +e
 if test "$mode" = eof; then
  RUBIX_CONTAINER_FIXTURE_PROTOCOL=1 "$binary" --container-mode </dev/null >/tmp/protocol-$mode.out 2>/tmp/protocol-$mode.err
 else
  printf X | RUBIX_CONTAINER_FIXTURE_PROTOCOL=1 "$binary" --container-mode >/tmp/protocol-$mode.out 2>/tmp/protocol-$mode.err
 fi
 code=$?
 set -e
 printf 'PROTOCOL_%s_BEGIN\nEXIT %s\n' "$mode" "$code"
 cat /tmp/protocol-$mode.out
 printf 'STDERR_BEGIN\n'; cat /tmp/protocol-$mode.err; printf 'STDERR_END\nPROTOCOL_%s_END\n' "$mode"
 test "$code" = 1
done
wait_event() {
 event=$1; file=$2; tries=0
 until grep -q '"event":"'"$event"'"' "$file"; do
  tries=$((tries+1))
  state=$(sed 's/^.*) //' /proc/$consumer/stat 2>/dev/null | cut -d ' ' -f 1) || state=missing
  if ! kill -0 "$consumer" 2>/dev/null || test "$state" = Z || test "$state" = missing || test "$tries" -ge 9000; then
   printf 'WAIT_EVENT_FAILED %s\nFAILED_CONSUMER_STDOUT_BEGIN\n' "$event"
   cat "$file"
   printf 'FAILED_CONSUMER_STDOUT_END\nFAILED_CONSUMER_STDERR_BEGIN\n'
   cat /tmp/container-$scenario.err
   printf 'FAILED_CONSUMER_STDERR_END\n'
   return 1
  fi
  sleep 0.02
 done
}
observe() {
 phase=$1; root=$2
 printf 'OBS_%s_BEGIN\nPID %s\n' "$phase" "$consumer"
 frame STAT /proc/$consumer/stat
 printf 'EXE '; sha256sum /proc/$consumer/exe
 printf 'MNT_NS %s\n' "$(readlink /proc/$consumer/ns/mnt)"
 printf 'CGROUP_NS %s\n' "$(readlink /proc/$consumer/ns/cgroup)"
 printf 'OBSERVER_MNT_NS %s\n' "$(readlink /proc/$$/ns/mnt)"
 printf 'OBSERVER_CGROUP_NS %s\n' "$(readlink /proc/$$/ns/cgroup)"
 printf 'GLOBAL_ROOT_ID %s\n' "$(stat -Lc '%d:%i' /sys/fs/cgroup)"
 printf 'ROOT_ID %s\n' "$(stat -Lc '%d:%i' "$root")"
 printf 'VISIBLE_ROOT_ID %s\n' "$(stat -Lc '%d:%i' /proc/$consumer/root/sys/fs/cgroup)"
 frame CANDIDATE_MOUNTS /proc/$consumer/mountinfo
 frame OBSERVER_MOUNTS /proc/self/mountinfo
 frame PROCESS_CGROUP /proc/$consumer/cgroup
 frame ROOT_TYPE "$root/cgroup.type"
 frame ROOT_MEMBERS "$root/cgroup.procs"
 frame AVAILABLE "$root/cgroup.controllers"
 frame ROOT_ENABLED "$root/cgroup.subtree_control"
 if test -d "$root/init"; then
  frame INIT_MEMBERS "$root/init/cgroup.procs"
  frame INIT_TYPE "$root/init/cgroup.type"
  frame INIT_ENABLED "$root/init/cgroup.subtree_control"
 else
  printf 'INIT_ABSENT\n'
 fi
 frame PARENT_ENABLED /sys/fs/cgroup/cgroup.subtree_control
 printf 'SIBLING_BEGIN\n'
 for file in cgroup.type cgroup.procs cgroup.subtree_control pids.max; do sha256sum /sys/fs/cgroup/rubix-container-sibling/$file; done
 printf 'SIBLING_END\nEXTERNAL_BEGIN\n'; sha256sum /tmp/external-runtime/*; printf 'EXTERNAL_END\nOBS_%s_END\n' "$phase"
}
# Each control FIFO is opened by the observer; only the candidate reads protocol bytes.
# Namespace setup is fixture-only and precedes READY. Observation releases each gate.
for scenario in signal real; do
 root=/sys/fs/cgroup/rubix-container-$scenario
 mkfifo /tmp/container-$scenario.control
 exec 3<>/tmp/container-$scenario.control
 sh "$bundle/launch.sh" "$root" <&3 >/tmp/container-$scenario.out 2>/tmp/container-$scenario.err &
 consumer=$!
 wait_event READY /tmp/container-$scenario.out
 observe "${scenario}_READY" "$root"
 if test "$scenario" = signal; then
  kill -TERM "$consumer"
 else
  printf G >&3
  wait_event FIRST /tmp/container-$scenario.out
  observe real_FIRST "$root"
  printf R >&3
  wait_event SECOND /tmp/container-$scenario.out
  observe real_SECOND "$root"
  printf Q >&3
 fi
 set +e
 wait "$consumer"
 code=$?
 set -e
 exec 3>&-
 printf 'CONSUMER_%s_BEGIN\nEXIT %s\nSTDOUT_BEGIN\n' "$scenario" "$code"
 cat /tmp/container-$scenario.out
 printf 'STDOUT_END\nSTDERR_BEGIN\n'; cat /tmp/container-$scenario.err
 printf 'STDERR_END\nCONSUMER_%s_END\n' "$scenario"
 if test "$scenario" = signal; then
  test "$code" = 1
  test ! -e "$root/init"
  test -z "$(cat "$root/cgroup.procs")"
  test -z "$(cat "$root/cgroup.subtree_control")"
  printf 'SIGNAL_NO_PREPARATION\n'
 else
  test "$code" = 0
  test -z "$(cat "$root/init/cgroup.procs")"
  test -z "$(cat "$root/cgroup.procs")"
  printf 'REAL_PROCESS_EXITED\n'
 fi
 test ! -e /proc/$consumer
 printf 'PID_ABSENT %s\n' "$consumer"
done
printf 'CONTAINER_GUEST_COMPLETE\n'
