#!/usr/bin/env python3
"""Explicitly authorized disposable Linux VM for the shared parity suite."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import resource
import secrets
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import threading
import urllib.request
import uuid

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "tools/parity"))

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

baseline = load("alpine_baseline_capture", ROOT / "tools/parity/fixtures/alpine-preparation/capture.py")
LIMIT = baseline.LIMIT
digest = baseline.digest
bounded_output = baseline.bounded_output
publish_cache_bytes = baseline.publish_cache_bytes
finish = baseline.finish

def validate_artifact(directory):
    import verify_build
    verify_build.verify(directory)
    return directory / "prepare_node_host"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-privileged-vm", action="store_true", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--image-cache", required=True, type=Path)
    parser.add_argument("--input-cache", type=Path, required=True)
    parser.add_argument("--inject-failure", choices=("setup", "test"))
    parser.add_argument("--artifact-directory", required=True, type=Path)
    args = parser.parse_args()
    artifact = validate_artifact(args.artifact_directory)
    import verify_build
    artifact_metadata = verify_build.verify(args.artifact_directory)
    dirty = subprocess.check_output(["git", "-C", str(ROOT), "status", "--porcelain", "--untracked-files=all"], timeout=10, text=True)
    if dirty.strip(): raise ValueError("capture requires clean committed source")
    revision = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], timeout=10, text=True).strip()
    source_hashes = {name: digest(HERE / name) for name in ("capture.py", "inputs.json", "guest.sh", "guest.py", "verify.py", "support.py", "verify_build.py", "namespace.sh", "guard-failure.sh", "module-wait.sh", "test_verify.py")}
    report = {"schema_version": 1, "status": "failed", "cases": [], "errors": [],
              "adapter": "qemu-disposable-node-constrained", "explicit_privilege_opt_in": True,
              "unsupported_capabilities": ["full-cluster-assets", "release-platform-qualification"],
              "source_sha256": source_hashes,
              "inherited_vm_sha256": digest(ROOT / "tools/parity/vm/run.py"),
              "baseline_helper_sha256": digest(ROOT / "tools/parity/fixtures/alpine-preparation/capture.py"),
              "build_verifier_sha256": digest(HERE / "verify_build.py"),
              "build_support_sha256": digest(HERE / "support.py"),
              "approved_build_sources": {name:digest(ROOT / "tools/node-container" / name) for name in ("verify_build.py", "support.py", "Build.Dockerfile", "build.py", "test-inventory.json")},
              "artifact_sha256": artifact_metadata["files"]["prepare_node_host"]["sha256"]}
    vm = None
    serial = None
    private = None
    log_thread = None
    ssh = None
    qmp = None
    private_secrets = []

    def command(label, argv, timeout=30, data=None, required=True):
        out = args.output / f"{label}.stdout"
        err = args.output / f"{label}.stderr"
        with out.open("wb") as stdout, err.open("wb") as stderr:
            process = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=stdout, stderr=stderr,
                                       start_new_session=True, preexec_fn=bounded_output)
            try:
                process.communicate(data, timeout=timeout)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.communicate(timeout=5)
                raise RuntimeError(f"{label} timed out")
        if out.stat().st_size >= LIMIT or err.stat().st_size >= LIMIT:
            raise RuntimeError(f"{label} reached output limit")
        if required and process.returncode:
            raise RuntimeError(f"{label} failed with exit {process.returncode}")
        return process.returncode, out.read_text(errors="replace"), err.read_text(errors="replace")

    args.output.mkdir(parents=True, exist_ok=False)
    os.chmod(args.output, 0o700)
    try:
        build = args.output / "artifact-build"
        build.mkdir()
        for name in ("artifact.json", "receipt.json", "source-hashes.json", "run.log", "build.log"):
            shutil.copyfile(args.artifact_directory / name, build / name)
        if platform.system() != "Darwin" or platform.machine() != "arm64":
            raise RuntimeError("initial VM adapter requires Darwin arm64 with HVF")
        report["revision"] = revision
        report["working_tree_snapshot"] = False
        inputs = json.loads((HERE / "inputs.json").read_text())
        report["inputs"] = inputs
        if args.input_cache:
            if args.input_cache.is_symlink(): raise ValueError("input cache symlink")
            for name, pin in inputs["packages"]["selected"].items():
                path = args.input_cache / "packages" / name
                if path.is_symlink() or not path.is_file() or path.stat().st_size != pin["bytes"] or digest(path) != pin["sha256"]: raise ValueError("package pin mismatch")
            index = args.input_cache / "packages/APKINDEX.tar.gz"
            if index.is_symlink() or not index.is_file() or index.stat().st_size > 8 * 1024 * 1024 or digest(index) != inputs["packages"]["index_sha256"]: raise ValueError("index pin mismatch")

        for executable in ("qemu-system-aarch64", "qemu-img", "ssh", "ssh-keygen", "scp"):
            if not shutil.which(executable):
                raise RuntimeError(f"missing required tool: {executable}")
        report["tools"] = {name: {"path": str(Path(shutil.which(name)).resolve()),
                                  "sha256": digest(Path(shutil.which(name)).resolve())}
                           for name in ("qemu-system-aarch64", "qemu-img", "ssh", "ssh-keygen", "scp")}
        firmware = Path("/opt/homebrew/share/qemu/edk2-aarch64-code.fd")
        variables = Path("/opt/homebrew/share/qemu/edk2-arm-vars.fd")
        report["firmware"] = {"code_sha256": digest(firmware), "vars_template_sha256": digest(variables)}
        if {name: tool["sha256"] for name, tool in report["tools"].items()} != inputs["host_tools"]:
            raise ValueError("host tool pin mismatch")
        if report["firmware"] != inputs["firmware"]:
            raise ValueError("firmware pin mismatch")
        _, version, _ = command("qemu-version", ["qemu-system-aarch64", "--version"])
        report["qemu_version"] = version
        if args.image_cache.is_symlink():
            raise RuntimeError("input cache directory must not be a symlink")
        args.image_cache.mkdir(parents=True, exist_ok=True)
        image = args.image_cache / "alpine-3.24.2-aarch64-cloudinit-r0.qcow2"
        if image.is_symlink():
            raise RuntimeError("cloud image cache entry must not be a symlink")
        if not image.exists():
            with tempfile.NamedTemporaryFile(dir=args.image_cache, prefix="download-", delete=False) as temporary:
                partial = Path(temporary.name)
                try:
                    download_began = time.monotonic()
                    downloaded = 0
                    with urllib.request.urlopen(inputs["image"]["url"], timeout=60) as response:
                        while chunk := response.read(1024 * 1024):
                            downloaded += len(chunk)
                            if downloaded > 1024 * 1024 * 1024 or time.monotonic() - download_began > 600:
                                raise RuntimeError("cloud image download exceeded size/time budget")
                            temporary.write(chunk)
                    temporary.flush()
                    if digest(partial, "sha512") != inputs["image"]["sha512"]:
                        raise RuntimeError("downloaded cloud image digest mismatch")
                    partial.replace(image)
                finally:
                    if partial.exists():
                        partial.unlink()
        if not image.is_file() or image.stat().st_size != inputs["image"]["bytes"] or digest(image, "sha512") != inputs["image"]["sha512"]:
            raise RuntimeError("cached cloud image digest mismatch")
        wheel = args.image_cache / "pycdlib-1.14.0-py2.py3-none-any.whl"
        if wheel.is_symlink():
            raise RuntimeError("seed builder cache entry must not be a symlink")
        if not wheel.exists():
            with urllib.request.urlopen(inputs["seed_builder"]["url"], timeout=60) as response:
                content = response.read(1024 * 1024)
            if hashlib.sha256(content).hexdigest() != inputs["seed_builder"]["sha256"]:
                raise RuntimeError("seed builder download digest mismatch")
            publish_cache_bytes(wheel, content, inputs["seed_builder"]["sha256"])
        if not wheel.is_file() or wheel.stat().st_size > 1024 * 1024 or digest(wheel) != inputs["seed_builder"]["sha256"]:
            raise RuntimeError("cached seed builder digest mismatch")
        sys.path.insert(0, str(wheel.resolve()))
        import pycdlib
        # A fresh private directory is the complete VM-owned writable filesystem set.
        private = Path(tempfile.mkdtemp(prefix="rubix-vm-", dir="/tmp"))
        report["owned_temporary_directory"] = str(private)
        shutil.copyfile(variables, private / "vars.fd")
        command("overlay", ["qemu-img", "create", "-f", "qcow2", "-F", "qcow2", "-b",
                            str(image.resolve()), str(private / "disk.qcow2"), "8G"])
        for name in ("client", "host"):
            command(f"key-{name}", baseline.keygen_arguments(name, private / name))
        seed = private / "seed"
        seed.mkdir()
        host_private = (private / "host").read_text()
        host_public = (private / "host.pub").read_text().strip()
        client_public = (private / "client.pub").read_text().strip()
        (seed / "meta-data").write_text(f"instance-id: rubix-{uuid.uuid4().hex}\nlocal-hostname: rubix-parity\n")
        guest_password = secrets.token_urlsafe(48)
        private_secrets = [guest_password.encode(), host_private.encode(), (private / "client").read_bytes()]
        report["guest_host_public_key"] = host_public
        report["owned_resources"] = ["disk.qcow2", "vars.fd", "seed.iso", "seed", "client", "client.pub", "host", "host.pub", "known_hosts", "qmp.sock"]
        (seed / "user-data").write_text(
            "#cloud-config\ndisable_root: false\nssh_pwauth: false\nusers:\n"
            "  - name: root\n    lock_passwd: false\n    plain_text_passwd: " + guest_password + "\n    ssh_authorized_keys:\n      - " + client_public + "\n"
            "ssh_keys:\n  ed25519_private: |\n" + "".join("    " + line + "\n" for line in host_private.splitlines()) +
            "  ed25519_public: " + host_public + "\n")
        iso = pycdlib.PyCdlib()
        iso.new(interchange_level=3, joliet=3, rock_ridge="1.09", vol_ident="cidata")
        for filename, iso_name in (("user-data", "USERDATA"), ("meta-data", "METADATA")):
            iso.add_file(str(seed / filename), iso_path=f"/{iso_name}.;1",
                         joliet_path=f"/{filename}", rr_name=filename)
        iso.write(str(private / "seed.iso"))
        iso.close()
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        report["ssh_forward"] = f"127.0.0.1:{port}"
        (private / "known_hosts").write_text(f"[127.0.0.1]:{port} {host_public}\n")
        ssh_options = ["-F", "/dev/null", "-i", str(private / "client"), "-o", "BatchMode=yes",
                       "-o", "IdentitiesOnly=yes", "-o", "IdentityAgent=none", "-o", "GlobalKnownHostsFile=/dev/null",
                       "-o", "StrictHostKeyChecking=yes", "-o",
                       f"UserKnownHostsFile={private / 'known_hosts'}", "-o", "ConnectTimeout=3",
                       "-o", "ConnectionAttempts=1", "-o", "ServerAliveInterval=5", "-o", "ServerAliveCountMax=2"]
        ssh = ["ssh", *ssh_options, "-p", str(port), "root@127.0.0.1"]
        qmp = private / "qmp.sock"
        qemu = ["qemu-system-aarch64", "-machine", "virt,accel=hvf", "-cpu", "host", "-smp", "2", "-m", "2048",
                "-display", "none", "-serial", "stdio", "-monitor", "none",
                "-qmp", f"unix:{qmp},server=on,wait=off", "-drive",
                f"if=pflash,format=raw,readonly=on,file={firmware}", "-drive",
                f"if=pflash,format=raw,file={private / 'vars.fd'}", "-drive",
                f"if=virtio,format=qcow2,file={private / 'disk.qcow2'}", "-drive",
                f"if=virtio,format=raw,readonly=on,file={private / 'seed.iso'}", "-netdev",
                f"user,id=n0,restrict=on,hostfwd=tcp:127.0.0.1:{port}-:22", "-device", "virtio-net-pci,netdev=n0"]
        report["qemu_argv"] = qemu
        serial = (args.output / "serial.log").open("wb")
        vm = subprocess.Popen(qemu, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        def drain_console():
            written = 0
            while chunk := vm.stdout.read1(4096):
                room = max(0, 1024 * 1024 - written)
                serial.write(chunk[:room])
                serial.flush()
                written += min(room, len(chunk))
                if len(chunk) > room:
                    report["serial_log_truncated"] = True
        log_thread = threading.Thread(target=drain_console, daemon=True)
        log_thread.start()
        report["owned_pid"] = vm.pid
        began = time.monotonic()
        until = began + 180
        while time.monotonic() < until:
            if vm.poll() is not None:
                raise RuntimeError(f"QEMU exited during boot: {vm.returncode}")
            code, _, _ = command("ssh-readiness", [*ssh, "true"], timeout=8, required=False)
            if code == 0:
                break
            time.sleep(1)
        else:
            raise RuntimeError("SSH readiness exceeded 180 seconds")
        report["ssh_readiness_seconds"] = time.monotonic() - began
        cloud_code, cloud_output, _ = command("cloud-init", [*ssh, "cloud-init status --wait --long --format json"], timeout=60, required=False)
        report["cloud_init_exit"] = cloud_code
        report["cloud_init"] = json.loads(cloud_output)
        baseline.validate_cloud_init(cloud_code, report["cloud_init"])
        _, environment, _ = command("environment", [*ssh, "uname -a; id; cat /etc/os-release; cat /proc/self/cgroup"])
        report["environment"] = environment
        # Prove privileged host preparation in this new guest only, then revert it.
        command("privileged-probe", [*ssh, "mkdir -p /mnt/rubix-probe && mount -t tmpfs -o size=1m tmpfs /mnt/rubix-probe && umount /mnt/rubix-probe && rmdir /mnt/rubix-probe"])
        report["privileged_mount_probe"] = "passed"
        if args.inject_failure == "setup":
            raise RuntimeError("intentional setup failure after guest privilege probe")
        if args.input_cache:
            bundle = private / "bundle"
            package_dir = bundle / "repo/aarch64"
            package_dir.mkdir(parents=True)
            for name in [*inputs["packages"]["selected"], "APKINDEX.tar.gz"]:
                shutil.copyfile(args.input_cache / "packages" / name, package_dir / name)
            for name in verify_build.BINARIES:
                shutil.copyfile(args.artifact_directory/name, bundle/name)
            for name in ("guest.py", "namespace.sh", "guard-failure.sh", "module-wait.sh"):
                shutil.copyfile(HERE / name, bundle / name)
            command("copy-inputs", ["scp", *ssh_options, "-P", str(port), "-r", str(bundle), "root@127.0.0.1:/tmp/rubix-bundle"], timeout=60)
            expected = {name:digest(HERE / name) for name in ("guest.py", "namespace.sh", "guard-failure.sh", "module-wait.sh")}
            expected["repo/aarch64/APKINDEX.tar.gz"]=inputs["packages"]["index_sha256"]
            expected.update({name:metadata["sha256"] for name,metadata in artifact_metadata["files"].items()})
            expected.update({"repo/aarch64/"+name:pin["sha256"] for name,pin in inputs["packages"]["selected"].items()})
            checks = "".join(value+"  "+name+"\n" for name,value in sorted(expected.items()))
            command("verify-guest-inputs", [*ssh, "cd /tmp/rubix-bundle && sha256sum -c - && chmod 0755 prepare_node_host rubix_kube host_preparation host_network"], data=checks.encode())
        _, observation, _ = command("constrained-cases", [*ssh, "RUBIX_RUN_PREPARATION="+str(int(bool(args.input_cache)))+" sh -s"], timeout=600, data=(HERE / "guest.sh").read_bytes())
        report["observation"] = observation
        if args.inject_failure == "test":
            raise RuntimeError("intentional test failure after diagnostics")
        import verify
        verify.semantic(observation,artifact_metadata)
        if {name: digest(HERE / name) for name in source_hashes} != source_hashes:
            raise ValueError("harness changed during capture")
        validate_artifact(args.artifact_directory)
        if subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], timeout=10, text=True).strip() != revision:
            raise ValueError("HEAD changed during capture")
        report["status"] = "passed"
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        report["errors"].append(str(error))
    finally:
        # Alpine may not run acpid. Request guest poweroff before inherited escalation.
        if vm is not None and vm.poll() is None and ssh is not None:
            try:
                command("guest-poweroff", [*ssh, "poweroff"], timeout=10, required=False)
                vm.wait(timeout=30)
                report["shutdown"] = "guest-poweroff"
            except Exception as error:
                report["errors"].append("guest poweroff: " + str(error))
        finish(report, args.output, vm, serial, private, log_thread, ssh, qmp, command, private_secrets)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
