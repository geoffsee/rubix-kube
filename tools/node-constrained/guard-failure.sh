#!/bin/sh
# Explicit guard double, never evidence about actual iptables capability.
printf 'GUARD_DOUBLE\n' >> /tmp/constrained-guard-double.calls
exit 73
