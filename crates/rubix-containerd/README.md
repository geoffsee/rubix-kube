# rubix-containerd

Managed containerd runtime configuration, shim resolution, and lifecycle integration for Rubix Kube.

## Features

- **TOML Configuration Generator**: Generates containerd v3 configuration matching official KubeSolo contracts (`version = 3`, `io.containerd.runc.v2`, `crun` binary options, CRI images and runtime plugins).
- **Shim Resolution**: Enforces `runtime_type = "io.containerd.runc.v2"` without `runtime_path` overrides, building execution `PATH` to resolve `containerd-shim-runc-v2` cleanly.
- **Snapshotter Detection**: Detects OverlayFS mounts via statfs magic (`0x794c7630`) and dynamically chooses `overlayfs`, `fuse-overlayfs`, or `native`.
- **Registry Proxies & Mirrors**: Supports containerd `hosts.toml` configuration directory model, ensuring user snippets survive configuration regeneration and service restarts.
- **Read-Only / Immutable Link Resilience**: Preserves existing correct symlinks on read-only filesystems.
- **Custom Root Isolation**: Fully self-contained execution without requiring global `/usr/local/bin` or `/opt/cni/bin` symlinks.

## Documentation

- [CONTAINERD_CONFIG.md](CONTAINERD_CONFIG.md) - Detailed specification of configuration format, shim resolution, and snapshotter selection.
