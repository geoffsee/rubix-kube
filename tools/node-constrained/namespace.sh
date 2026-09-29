#!/bin/sh
# Executed only within a fresh owned guest and a recursively private mount namespace.
set -eu
case_name=$1
bundle=/tmp/rubix-bundle
if grep -Eq ' (shared|master|propagate_from):' /proc/self/mountinfo; then exit 1; fi
# MAKE_TOOL_VIEW_BEGIN
make_tool_view() {
 source_dir=$(readlink -f "$1") || return 1
 reference=$2
 view=$3
 tool=$4
 double=$5
 case "$tool" in iptables|modprobe) ;; *) return 1 ;; esac
 test -d "$source_dir" && test -d "$reference" || return 1
 test -f "$double" && test "$(wc -c < "$double")" -le 4096 || return 1
 mkdir "$view" || return 1
 count=0
 # Resolve before overlay: ../bin aliases must retain their original meaning.
 # Canonical internal targets use the private original bind, never the new view.
 for entry in "$source_dir"/* "$source_dir"/.[!.]* "$source_dir"/..?*; do
  if ! test -e "$entry" && ! test -L "$entry"; then continue; fi
  count=$((count+1))
  test "$count" -le 256 || return 1
  resolved=$(readlink -f "$entry") || return 1
  test -f "$resolved" || return 1
  case "$resolved" in
   "$source_dir"/*) target="$reference/${resolved#"$source_dir"/}" ;;
   *) target=$resolved ;;
  esac
  ln -s "$target" "$view/${entry##*/}" || return 1
 done
 # Remove the private alias itself; never follow it while replacing the double.
 rm -f "$view/$tool" || return 1
 cp "$double" "$view/$tool" || return 1
 chmod 0755 "$view/$tool"
}
# MAKE_TOOL_VIEW_END
case "$case_name" in
 guard|cancel)
  reference=/tmp/constrained-$case_name-original
  view=/tmp/constrained-$case_name-bin
  mkdir "$reference"
  mount --bind /usr/sbin "$reference"
  mount -o remount,bind,ro "$reference"
  if test "$case_name" = guard; then tool=iptables; double=guard-failure.sh; else tool=modprobe; double=module-wait.sh; fi
  make_tool_view /usr/sbin "$reference" "$view" "$tool" "$bundle/$double"
  mount --bind "$view" /usr/sbin
  ;;
 correct|needs_write) ;;
 *) exit 1 ;;
esac
mount --bind /proc/sys /proc/sys
mount -o remount,bind,ro /proc/sys
exec env RUBIX_CONTAINER_FIXTURE_PROTOCOL=1 "$bundle/prepare_node_host" --no-container-mode --disable-ipv6 --container-runtime-endpoint=unix:///tmp/external-runtime/containerd.sock
