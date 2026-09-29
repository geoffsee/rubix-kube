#!/bin/sh
# Explicit cancellation double. exec retains PID; supervisor owns this sleep/group.
printf '%s %s\n' "$$" "$1" > /tmp/constrained-module-double.pid
exec /bin/sleep 60
