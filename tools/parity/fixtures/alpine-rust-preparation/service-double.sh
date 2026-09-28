#!/sbin/openrc-run
# Deliberate negative fixture: command succeeds without repairing controller state.
start() { return 0; }
