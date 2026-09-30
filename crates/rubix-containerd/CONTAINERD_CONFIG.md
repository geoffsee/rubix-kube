# Containerd Managed Runtime Configuration

This document specifies how `rubix-containerd` configures the managed containerd daemon, resolves shims, manages registry proxies, and handles custom root installations.

---

## 1. Architectural Principles

1. **Self-Contained Roots**: No dependencies on global host symlinks in `/usr/local/bin` (for `runc` or `containerd-shim-runc-v2`) or `/opt/cni/bin` (for CNI plugins). All runtime artifacts, binaries, and state reside inside the managed base path (e.g. `/var/lib/kubesolo/containerd` or custom path).
2. **Standard Shim Resolution via `PATH`**:
   - `runtime_type` is configured as the registered containerd type `"io.containerd.runc.v2"`.
   - `runtime_path` is deliberately omitted. Setting `runtime_path` forces containerd to run a spurious `<shim> -info` probe that looks for `runc` (which is not shipped; `crun` is used instead).
   - containerd resolves the extracted `containerd-shim-runc-v2` by having its binary directory prepended to `PATH` in the execution environment.
3. **crun as OCI Runtime**: Configured under `options.BinaryName` pointing to the embedded or extracted `crun` binary.
4. **Registry Hosts Separation**: Registry mirrors, pull-through proxies, and TLS configurations use containerd's `hosts.toml` directory model under `containerd/registry/<host>/hosts.toml`. These files are preserved across restarts and configuration regenerations.

---

## 2. Configuration Format (TOML Version 3)

```toml
version = 3
root = "/var/lib/kubesolo/containerd/root"
state = "/var/lib/kubesolo/containerd/state"
imports = ["/etc/containerd/config.d/*.toml"]

[grpc]
address = "/var/lib/kubesolo/containerd/containerd.sock"

[plugins."io.containerd.cri.v1.images"]
image_pull_progress_timeout = "2m0s"

[plugins."io.containerd.cri.v1.images".pinned_images]
sandbox = "portainer/pause:latest"

[plugins."io.containerd.cri.v1.images".registry]
config_path = "/var/lib/kubesolo/containerd/registry"

[plugins."io.containerd.cri.v1.runtime.cni"]
bin_dirs = ["/var/lib/kubesolo/containerd/cni/plugins"]
conf_dir = "/etc/cni/net.d"

[plugins."io.containerd.cri.v1.runtime.containerd"]
default_runtime_name = "crun"

[plugins."io.containerd.cri.v1.runtime.containerd.runtimes.crun"]
runtime_type = "io.containerd.runc.v2"
snapshotter = "overlayfs"

[plugins."io.containerd.cri.v1.runtime.containerd.runtimes.crun.options]
BinaryName = "/var/lib/kubesolo/containerd/crun"
SystemdCgroup = false

[plugins."io.containerd.runtime.v2.task"]
platforms = ["linux/amd64", "linux/arm64", "linux/arm"]
```

---

## 3. Snapshotter Selection Matrix

When selecting the snapshotter for containerd's root filesystem:

| Condition | Selected Snapshotter | Rationale |
| :--- | :--- | :--- |
| Normal root filesystem | `overlayfs` | Kernel overlayfs provides the highest performance CoW layering. |
| OverlayFS root (e.g. Alpine live-boot) with `fuse-overlayfs` in `PATH` | `fuse-overlayfs` | Kernel overlay-on-overlay returns `EINVAL` from `mount(2)`. Userspace FUSE overlay preserves copy-on-write semantics. |
| OverlayFS root without `fuse-overlayfs` in `PATH` | `native` | Safe universal fallback that copies layers instead of stacking them. |

Filesystem type detection checks for the Linux statfs magic `0x794c7630` (`OVERLAYFS_SUPER_MAGIC`).

---

## 4. Cgroup Driver Evaluation

Containerd and crun configure `SystemdCgroup = true` only when:
1. The host unified cgroup v2 hierarchy is present (`/sys/fs/cgroup/cgroup.controllers`).
2. Systemd is the active init system (`/run/systemd/private`).

On non-systemd systems (such as Alpine running OpenRC), `SystemdCgroup` is set to `false` even if cgroup v2 is active, falling back to crun's direct cgroupfs management.

---

## 5. Registry Configuration (`hosts.toml`)

Containerd's `config_path` is configured to `<base_path>/containerd/registry`.

Operators create per-registry directories with `hosts.toml`:

```text
/var/lib/kubesolo/containerd/registry/
├── docker.io/
│   └── hosts.toml
├── ghcr.io/
│   └── hosts.toml
└── _default/
    └── hosts.toml
```

Example `docker.io/hosts.toml`:

```toml
server = "https://registry-1.docker.io"

[host."https://mirror.corp.internal"]
  capabilities = ["pull", "resolve"]
  override_path = true
  ca = "/etc/ssl/certs/harbor-ca.crt"
```

These configuration files are read dynamically by containerd at pull time. The `rubix-containerd` generator and state cleanup routines guarantee that these files are never overwritten, truncated, or removed during configuration regeneration or daemon restarts.

---

## 6. Compatibility Paths

- **System containerd socket**: `/run/containerd/containerd.sock` is symlinked to the managed socket path for tooling compatibility (`crictl`, host utilities).
- **Immutable roots**: `ensure_symbolic_link` detects if an existing symlink already targets the desired source and avoids recreating it, ensuring smooth operation on read-only root filesystems.
