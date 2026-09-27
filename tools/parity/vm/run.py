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

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

BASE = load("parity_base", HERE.parent / "run.py")
ASSESS = load("parity_assertions", HERE.parent / "driver.py").assess
LIMIT = 256 * 1024


def digest(path, algorithm="sha256"):
    result = hashlib.new(algorithm)
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            result.update(chunk)
    return result.hexdigest()


def bounded_output():
    resource.setrlimit(resource.RLIMIT_FSIZE, (LIMIT, LIMIT))


def publish_cache_bytes(path, content, expected):
    if path.is_symlink():
        raise ValueError("cache entry must not be a symlink")
    if hashlib.sha256(content).hexdigest() != expected:
        raise ValueError("cache content digest mismatch")
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix="download-", delete=False) as stream:
        temporary = Path(stream.name)
        try:
            stream.write(content)
            stream.flush()
            temporary.replace(path)
        finally:
            if temporary.exists():
                temporary.unlink()


def classify_timeout(code):
    # GNU timeout reserves 124; SIGKILL (including its forced deadline) returns 137.
    # Neither may be converted to a pass by an expected-exit assertion.
    return code in (124, 137)


def run_cases(cases, command, ssh, output, report):
    for index, case in enumerate(cases):
        environment = {"PATH": "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin", "HOME": "/root", "LANG": "C.UTF-8"}
        environment.update(case.get("env", {}))
        argv = ["env", "-i", *(f"{key}={value}" for key, value in environment.items()),
                "timeout", "--signal=KILL", str(case.get("timeout_seconds", 30)), "/opt/rubix/artifact", *case["argv"]]
        guest_uid = 0
        if case.get("privilege", "none") == "none":
            argv = ["runuser", "--user", "nobody", "--", *argv]
            guest_uid = 65534
        case_began = time.monotonic()
        try:
            code, stdout, stderr = command(f"case-{index:03d}", [*ssh, shlex.join(argv)],
                                            timeout=case.get("timeout_seconds", 30) + 10,
                                            data=case.get("stdin", "").encode(), required=False)
        except RuntimeError as error:
            report["cases"].append({"id": case["id"], "status": "failed", "error": str(error),
                                    "duration_seconds": time.monotonic() - case_began})
            report["unexecuted_case_ids"] = [c["id"] for c in cases[index + 1:]]
            raise
        failures = ASSESS(case, code, stdout, stderr)
        timed_out = classify_timeout(code)
        if timed_out:
            failures.append("command reached reserved timeout/SIGKILL status; aborting VM suite")
        report["cases"].append({"id": case["id"], "status": "failed" if failures else "passed",
                                "exit_code": code, "failures": failures, "guest_uid": guest_uid,
                                "timed_out": timed_out, "duration_seconds": time.monotonic() - case_began,
                                "requested_privilege": case.get("privilege", "none"),
                                "stdout_sha256": digest(output / f"case-{index:03d}.stdout"),
                                "stderr_sha256": digest(output / f"case-{index:03d}.stderr")})
        if timed_out:
            report["unexecuted_case_ids"] = [c["id"] for c in cases[index + 1:]]
            break


def finish(report, output, vm, serial, private, log_thread, ssh, qmp, command):
    def cleanup_attempt(label, operation):
        try:
            operation()
            return True
        except Exception as error:
            report["errors"].append(f"{label}: {error}")
            return False

    if vm and vm.poll() is None:
        cleanup_attempt("diagnostics", lambda: command("guest-diagnostics", [*ssh,
            "journalctl -n 100 --no-pager; ps -eo pid,ppid,uid,comm"], timeout=10, required=False))
        def powerdown():
            with socket.socket(socket.AF_UNIX) as control:
                control.settimeout(3)
                control.connect(str(qmp))
                control.recv(4096)
                control.sendall(b'{"execute":"qmp_capabilities"}\n')
                control.recv(4096)
                control.sendall(b'{"execute":"system_powerdown"}\n')
            vm.wait(timeout=30)
            report["shutdown"] = "acpi-powerdown"
        if not cleanup_attempt("ACPI powerdown", powerdown):
            report["shutdown"] = "forced-process-termination"
            def signal_group(value):
                try:
                    os.killpg(vm.pid, value)
                except ProcessLookupError:
                    pass
            cleanup_attempt("QEMU terminate", lambda: signal_group(signal.SIGTERM))
            if not cleanup_attempt("QEMU terminate wait", lambda: vm.wait(timeout=5)):
                cleanup_attempt("QEMU kill", lambda: signal_group(signal.SIGKILL))
                cleanup_attempt("QEMU kill wait", lambda: vm.wait(timeout=5))
    if vm:
        report["qemu_exit_code"] = vm.poll()
        if vm.returncode != 0:
            report["errors"].append(f"QEMU exited abnormally: {vm.returncode}")
        def ensure_group_absent():
            try:
                os.killpg(vm.pid, 0)
            except ProcessLookupError:
                report["owned_process_group_absent"] = True
                return
            report["errors"].append("owned QEMU process group required forced cleanup")
            os.killpg(vm.pid, signal.SIGKILL)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                try:
                    os.killpg(vm.pid, 0)
                except ProcessLookupError:
                    report["owned_process_group_absent"] = True
                    return
                time.sleep(0.05)
            raise RuntimeError("owned QEMU process group remains")
        cleanup_attempt("QEMU process-group check", ensure_group_absent)
    if log_thread:
        cleanup_attempt("console drain", lambda: log_thread.join(timeout=5))
    if serial:
        if log_thread and log_thread.is_alive():
            report["errors"].append("console reader did not finish")
        else:
            cleanup_attempt("console close", serial.close)
    if private:
        if vm is None or (vm.poll() is not None and report.get("owned_process_group_absent")):
            cleanup_attempt("private directory removal", lambda: shutil.rmtree(private))
            report["owned_temporary_directory_removed"] = not private.exists()
        else:
            report["owned_temporary_directory_removed"] = False
            report["errors"].append("VM files retained because owned processes are not confirmed absent")
    if report["errors"]:
        report["status"] = "failed"
    descriptor = os.open(output / "result.json", os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, "w") as output:
        json.dump(report, output, indent=2)
        output.write("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-privileged-vm", action="store_true", required=True)
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--suite", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--image-cache", required=True, type=Path)
    parser.add_argument("--inject-failure", choices=("setup", "test"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    os.chmod(args.output, 0o700)
    report = {"schema_version": 1, "status": "failed", "cases": [], "errors": [],
              "adapter": "qemu-disposable-linux-vm", "explicit_privilege_opt_in": True,
              "unsupported_capabilities": ["full-cluster-assets", "release-platform-qualification"],
              "source_sha256": {name: digest(HERE / name) for name in ("run.py", "inputs.json")},
              "shared_assertions_sha256": digest(HERE.parent / "driver.py"),
              "shared_validation_sha256": digest(HERE.parent / "run.py")}
    vm = None
    serial = None
    private = None
    log_thread = None
    ssh = None
    qmp = None

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

    try:
        if platform.system() != "Darwin" or platform.machine() != "arm64":
            raise RuntimeError("initial VM adapter requires Darwin arm64 with HVF")
        artifact = json.loads(args.artifact.read_text())
        suite = json.loads(args.suite.read_text())
        BASE.validate(artifact, suite)
        binary = (args.artifact.parent / artifact["binary"]).resolve()
        if digest(binary) != artifact["sha256"]:
            raise RuntimeError("artifact digest mismatch")
        inputs = json.loads((HERE / "inputs.json").read_text())
        report.update(artifact=artifact, suite_id=suite["id"], suite_sha256=digest(args.suite), inputs=inputs)
        for executable in ("qemu-system-aarch64", "qemu-img", "ssh", "ssh-keygen", "scp"):
            if not shutil.which(executable):
                raise RuntimeError(f"missing required tool: {executable}")
        report["tools"] = {name: {"path": str(Path(shutil.which(name)).resolve()),
                                  "sha256": digest(Path(shutil.which(name)).resolve())}
                           for name in ("qemu-system-aarch64", "qemu-img", "ssh", "ssh-keygen", "scp")}
        firmware = Path("/opt/homebrew/share/qemu/edk2-aarch64-code.fd")
        variables = Path("/opt/homebrew/share/qemu/edk2-arm-vars.fd")
        report["firmware"] = {"code_sha256": digest(firmware), "vars_template_sha256": digest(variables)}
        _, version, _ = command("qemu-version", ["qemu-system-aarch64", "--version"])
        report["qemu_version"] = version
        if args.image_cache.is_symlink():
            raise RuntimeError("input cache directory must not be a symlink")
        args.image_cache.mkdir(parents=True, exist_ok=True)
        image = args.image_cache / "debian-12-genericcloud-arm64-20260923-2610.qcow2"
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
            command(f"key-{name}", ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(private / name)])
        seed = private / "seed"
        seed.mkdir()
        host_private = (private / "host").read_text()
        host_public = (private / "host.pub").read_text().strip()
        client_public = (private / "client.pub").read_text().strip()
        (seed / "meta-data").write_text(f"instance-id: rubix-{uuid.uuid4().hex}\nlocal-hostname: rubix-parity\n")
        (seed / "user-data").write_text(
            "#cloud-config\ndisable_root: false\nssh_pwauth: false\nusers:\n"
            "  - name: root\n    ssh_authorized_keys:\n      - " + client_public + "\n"
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
            while chunk := vm.stdout.read(4096):
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
        command("cloud-init", [*ssh, "cloud-init status --wait"], timeout=60)
        _, environment, _ = command("environment", [*ssh, "uname -a; id; cat /etc/os-release; cat /proc/self/cgroup"])
        report["environment"] = environment
        # Prove privileged host preparation in this new guest only, then revert it.
        command("privileged-probe", [*ssh, "mkdir -p /mnt/rubix-probe && mount -t tmpfs -o size=1m tmpfs /mnt/rubix-probe && umount /mnt/rubix-probe && rmdir /mnt/rubix-probe"])
        report["privileged_mount_probe"] = "passed"
        if args.inject_failure == "setup":
            raise RuntimeError("intentional setup failure after guest privilege probe")
        command("copy-artifact", ["scp", *ssh_options, "-P", str(port), str(binary), "root@127.0.0.1:/tmp/artifact"], timeout=60)
        _, observed_digest, _ = command("artifact-digest", [*ssh, "mkdir -p /opt/rubix && mv /tmp/artifact /opt/rubix/artifact && chmod 0755 /opt/rubix/artifact && sha256sum /opt/rubix/artifact"])
        if observed_digest.split()[0] != artifact["sha256"]:
            raise RuntimeError("guest artifact digest mismatch")
        run_cases(suite["cases"], command, ssh, args.output, report)
        if args.inject_failure == "test":
            raise RuntimeError("intentional test failure after case diagnostics")
        report["status"] = "failed" if any(c["status"] == "failed" for c in report["cases"]) else "passed"
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        report["errors"].append(str(error))
    finally:
        finish(report, args.output, vm, serial, private, log_thread, ssh, qmp, command)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
