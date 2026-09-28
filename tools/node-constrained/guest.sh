#!/bin/sh
set -eu
ulimit -f 2048
printf '/tmp/rubix-bundle/repo\n' > /etc/apk/repositories
apk add --no-network nftables iptables util-linux-misc=2.42.3-r1 >&2
rc-service cgroups start >&2
printf 1 > /proc/sys/net/ipv4/ip_forward
modprobe xt_comment
exec python3 /tmp/rubix-bundle/guest.py
