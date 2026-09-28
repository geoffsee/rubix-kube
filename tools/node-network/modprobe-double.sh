#!/bin/sh
# Explicit disposable-guest double: never evidence of real module loading.
set -eu
test "$#" -eq 1
test "$PATH" = /usr/sbin:/usr/bin:/sbin:/bin
test "$LANG" = C
test "${PRIVATE_SENTINEL-unset}" = unset
test "$PWD" = /
printf '%s\n' "$1" >> /tmp/rubix-network-attempts
case "$(cat /tmp/rubix-network-mode)" in
 fail) exit 17 ;;
 limits)
  case "$1" in
   br_netfilter) exec /bin/sleep 30 ;;
   overlay) exec /bin/busybox head -c 4097 /dev/zero ;;
   *) exit 17 ;;
  esac ;;
 wait) printf 'ready\n' > /tmp/rubix-network-waiting; exec /bin/sleep 30 ;;
 *) exit 19 ;;
esac
