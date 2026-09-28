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
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT / "tools/parity"))

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

baseline = load("alpine_baseline_capture", HERE.parent / "alpine-preparation/capture.py")
LIMIT = baseline.LIMIT
digest = baseline.digest
bounded_output = baseline.bounded_output
publish_cache_bytes = baseline.publish_cache_bytes
finish = baseline.finish

def validate_artifact(directory):
    subprocess.run([sys.executable, str(HERE.parent / "prerequisite-preparation/verify_linux.py"), str(directory)], check=True, timeout=30, stdout=subprocess.DEVNULL)
    metadata = json.loads((directory / "artifact.json").read_text())
    artifact = directory / "rubixctl"
    if artifact.is_symlink() or artifact.stat().st_size != 5388504 or digest(artifact) != "1989a32474bcbc1e6064e0c7093903da769d240021d66b51aa1fa31ac47515fa":
        raise ValueError("Rust artifact pin mismatch")
    if metadata["sha256"] != digest(artifact) or metadata["size"] != artifact.stat().st_size or metadata["target"] != "aarch64-unknown-linux-musl":
        raise ValueError("artifact metadata mismatch")
    receipt = json.loads((directory / "receipt.json").read_text())
    expected_revision = json.loads((HERE / "inputs.json").read_text())["artifact"]["revision"]
    if metadata["revision"] != expected_revision or receipt["revision"] != expected_revision:
        raise ValueError("artifact build revision mismatch")
    return artifact

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
    revision = subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], timeout=10, text=True).strip()
    source_hashes = {name: digest(HERE / name) for name in ("capture.py", "inputs.json", "guest.sh", "reboot.sh", "verify.py", "service-double.sh")}
    report = {"schema_version": 1, "status": "failed", "cases": [], "errors": [],
              "adapter": "qemu-disposable-alpine-rust-preparation", "explicit_privilege_opt_in": True,
              "unsupported_capabilities": ["full-cluster-assets", "release-platform-qualification"],
              "source_sha256": source_hashes,
              "inherited_vm_sha256": digest(ROOT / "tools/parity/vm/run.py"),
              "baseline_helper_sha256": digest(HERE.parent / "alpine-preparation/capture.py"),
              "build_verifier_sha256": digest(HERE.parent / "prerequisite-preparation/verify_linux.py"),
              "build_support_sha256": digest(HERE.parent / "prerequisite-preparation/support.py"),
              "artifact_sha256": digest(artifact)}
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
        for name in ("artifact.json", "receipt.json", "source-hashes.json", "run.log"):
            shutil.copyfile(args.artifact_directory / name, build / name)
        if platform.system() != "Darwin" or platform.machine() != "arm64":
            raise RuntimeError("initial VM adapter requires Darwin arm64 with HVF")
        report["revision"] = revision
        report["working_tree_snapshot"] = True
        inputs = json.loads((HERE / "inputs.json").read_text())
        report["inputs"] = inputs
        if args.input_cache:
            if args.input_cache.is_symlink(): raise ValueError("input cache symlink")
            for name, pin in inputs["packages"]["selected"].items():
                path = args.input_cache / "packages" / name
                if path.is_symlink() or digest(path) != pin["sha256"]: raise ValueError("package pin mismatch")
            if digest(args.input_cache / "packages/APKINDEX.tar.gz") != inputs["packages"]["index_sha256"]: raise ValueError("index pin mismatch")
            if digest(artifact) != inputs["artifact"]["sha256"]: raise ValueError("oracle pin mismatch")
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
        if digest(image, "sha512") != inputs["image"]["sha512"]:
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
        if digest(wheel) != inputs["seed_builder"]["sha256"]:
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
            shutil.copyfile(artifact, bundle / "rubixctl")
            shutil.copyfile(HERE / "service-double.sh", bundle / "service-double.sh")
            command("copy-inputs", ["scp", *ssh_options, "-P", str(port), "-r", str(bundle), "root@127.0.0.1:/tmp/rubix-bundle"], timeout=60)
            expected = {"service-double.sh":digest(HERE / "service-double.sh"), "rubixctl":inputs["artifact"]["sha256"], "repo/aarch64/APKINDEX.tar.gz":inputs["packages"]["index_sha256"]}
            expected.update({"repo/aarch64/"+name:pin["sha256"] for name,pin in inputs["packages"]["selected"].items()})
            checks = "".join(value+"  "+name+"\n" for name,value in sorted(expected.items()))
            command("verify-guest-inputs", [*ssh, "cd /tmp/rubix-bundle && sha256sum -c - && chmod 0755 rubixctl"], data=checks.encode())
        _, observation, _ = command("baseline-inventory", [*ssh, "RUBIX_RUN_PREPARATION="+str(int(bool(args.input_cache)))+" sh -s"], timeout=180, data=(HERE / "guest.sh").read_bytes())
        report["observation"] = observation
        if args.input_cache:
            _, old_boot, _ = command("before-reboot-id", [*ssh, "cat /proc/sys/kernel/random/boot_id"])
            command("guest-reboot", [*ssh, "reboot"], required=False)
            until = time.monotonic() + 180
            while time.monotonic() < until:
                if vm.poll() is not None: raise RuntimeError("guest exited during reboot")
                code, current_boot, _ = command("reboot-readiness", [*ssh, "cat /proc/sys/kernel/random/boot_id"], timeout=8, required=False)
                if code == 0 and current_boot.strip() and current_boot != old_boot: break
                time.sleep(1)
            else: raise RuntimeError("reboot deadline")
            reboot_code, reboot_output, _ = command("reboot-cloud-init", [*ssh, "cloud-init status --wait --long --format json"], timeout=60, required=False)
            report["reboot_cloud_init_exit"] = reboot_code
            report["reboot_cloud_init"] = json.loads(reboot_output)
            baseline.validate_cloud_init(reboot_code, report["reboot_cloud_init"])
            command("reboot-verification", [*ssh, "sh -s"], timeout=60, data=(HERE / "reboot.sh").read_bytes())
        if args.inject_failure == "test":
            raise RuntimeError("intentional test failure after diagnostics")
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
