"""Distribution-neutral assertions executed inside an isolated Linux container."""
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import re
import signal
import subprocess
import time
import traceback


def validate_files(case):
    """Keep fixture paths canonical and bound their UTF-8 payload before staging."""
    files = case.get("files", {})
    if not isinstance(files, dict) or len(files) > 32:
        raise ValueError("case files must map at most 32 paths to UTF-8 strings")
    size = 0
    for name, content in files.items():
        if (not isinstance(name, str) or len(name) > 256
                or not re.fullmatch(r"[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*", name)
                or any(part in (".", "..") for part in name.split("/"))):
            raise ValueError("fixture path must be a canonical relative POSIX path")
        if not isinstance(content, str):
            raise ValueError("fixture content must be a UTF-8 string")
        size += len(content.encode("utf-8"))
        if any(parent in files for parent in (str(p) for p in Path(name).parents) if parent != "."):
            raise ValueError("fixture paths cannot overlap a file and directory")
    if size > 256 * 1024:
        raise ValueError("case fixture payload exceeds 256 KiB")
    fixture_argv(case, Path("/fixtures/validation"))
    return size


def fixture_argv(case, directory):
    """Expand only complete fixture tokens; never interpret shell expressions."""
    argv = []
    for argument in case["argv"]:
        if argument.startswith("{fixture:"):
            if not argument.endswith("}") or argument[9:-1] not in case.get("files", {}):
                raise ValueError("fixture token must name a supplied file")
            argument = str(directory / argument[9:-1])
        argv.append(argument)
    return argv


def stage_files(cases, root):
    """Stage trusted, validated suite inputs in a fresh owned image context."""
    root.mkdir()
    root.chmod(0o755)
    for index, case in enumerate(cases):
        validate_files(case)
        for name, content in case.get("files", {}).items():
            destination = root / f"{index:03d}" / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            for parent in destination.parents:
                if parent == root:
                    break
                parent.chmod(0o755)
            with destination.open("x", encoding="utf-8") as stream:
                stream.write(content)
            destination.chmod(0o444)


def assess(case, code, stdout, stderr):
    """Return assertion failures without consulting artifact identity."""
    failures = []
    expected = case["expect"]
    if code != expected["exit_code"]:
        failures.append(f"exit {code}, expected {expected['exit_code']}")
    for field, actual in [("stdout", stdout), ("stderr", stderr), ("combined", stdout + stderr)]:
        for value in expected.get(field + "_contains", []):
            if value not in actual:
                failures.append(f"{field} missing {value!r}")
        if field + "_equals" in expected and actual != expected[field + "_equals"]:
            failures.append(f"{field} differs from exact expectation")
    return failures


OUTPUT_LIMIT = 256 * 1024


def output_limits():
    resource.setrlimit(resource.RLIMIT_FSIZE, (OUTPUT_LIMIT, OUTPUT_LIMIT))


def execute(case, index, output):
    began = time.monotonic()
    record = {"id": case["id"], "argv": case["argv"], "status": "failed"}
    if case.get("privilege", "none") != "none":
        record.update(status="gap", reason="privileged Linux VM adapter is not implemented")
        return record
    environment = {"PATH": "/usr/local/bin:/usr/bin:/bin", "HOME": "/tmp", "LANG": "C.UTF-8"}
    environment.update(case.get("env", {}))
    stdout_path = output / f"{index:03d}.stdout"
    stderr_path = output / f"{index:03d}.stderr"
    process = None
    try:
        with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
            process = subprocess.Popen(["/artifact/executable", *fixture_argv(case, Path(f"/fixtures/{index:03d}"))],
                                       stdin=subprocess.PIPE, stdout=stdout, stderr=stderr,
                                       env=environment, cwd="/tmp", start_new_session=True,
                                       user=65532, group=65532, extra_groups=[], preexec_fn=output_limits)
            try:
                process.communicate(case.get("stdin", "").encode(), timeout=case.get("timeout_seconds", 30))
            except subprocess.TimeoutExpired:
                record["timeout"] = True
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.communicate(timeout=5)
        out = stdout_path.read_bytes()[:OUTPUT_LIMIT].decode(errors="replace")
        err = stderr_path.read_bytes()[:OUTPUT_LIMIT].decode(errors="replace")
        failures = assess(case, process.returncode, out, err)
        if record.get("timeout"):
            failures.append("command exceeded its timeout")
        if any(path.stat().st_size >= OUTPUT_LIMIT for path in (stdout_path, stderr_path)):
            failures.append("command reached its 256 KiB output limit")
        record.update(exit_code=process.returncode, failures=failures,
                      stdout_file=stdout_path.name, stderr_file=stderr_path.name,
                      stdout_sha256=hashlib.sha256(stdout_path.read_bytes()).hexdigest(),
                      stderr_sha256=hashlib.sha256(stderr_path.read_bytes()).hexdigest(),
                      status="failed" if failures else "passed")
    except (OSError, subprocess.SubprocessError) as error:
        record["error"] = str(error)
    finally:
        if process:
            try:
                os.killpg(process.pid, 0)
            except ProcessLookupError:
                record["owned_process_group_absent"] = True
            else:
                record["owned_process_group_absent"] = False
                record["status"] = "failed"
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
        # A child can create a new session; inspect the dedicated artifact UID too.
        escaped = []
        for path in Path("/proc").glob("[0-9]*/status"):
            try:
                fields = dict(line.split(":", 1) for line in path.read_text().splitlines() if ":" in line)
                if int(fields["Uid"].split()[0]) == 65532:
                    pid = int(path.parent.name)
                    escaped.append(pid)
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
            except (FileNotFoundError, ProcessLookupError):
                pass
        if escaped:
            record.update(status="failed", remaining_artifact_pids=escaped)
        # As container PID 1 the driver adopts orphan descendants and reaps them.
        until = time.monotonic() + 2
        while time.monotonic() < until:
            try:
                pid, _ = os.waitpid(-1, os.WNOHANG)
                if pid == 0:
                    time.sleep(0.02)
                    continue
            except ChildProcessError:
                break
        record["duration_seconds"] = time.monotonic() - began
    return record


def main():
    output = Path("/evidence")
    report = {"schema_version": 1, "status": "failed", "cases": [],
              "unsupported_capabilities": ["privileged-runtime", "cluster-lifecycle", "conformance"],
              "environment": {"system": platform.system(), "architecture": platform.machine(),
                              "kernel": platform.release(), "driver_uid": os.getuid(), "artifact_uid": 65532},
              "driver_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    try:
        artifact = json.loads(Path("/artifact/artifact.json").read_text())
        suite = json.loads(Path("/artifact/suite.json").read_text())
        report.update(artifact=artifact, suite_id=suite["id"],
                      suite_sha256=hashlib.sha256(Path("/artifact/suite.json").read_bytes()).hexdigest())
        actual = hashlib.sha256(Path("/artifact/executable").read_bytes()).hexdigest()
        if actual != artifact["sha256"]:
            raise RuntimeError("runtime artifact digest mismatch")
        if os.environ.get("PARITY_INJECT_FAILURE") == "setup":
            raise RuntimeError("intentional setup failure after isolated resources exist")
        for index, case in enumerate(suite["cases"]):
            outcome = execute(case, index, output)
            report["cases"].append(outcome)
            if outcome.get("remaining_artifact_pids") or outcome.get("owned_process_group_absent") is False:
                report["unexecuted_case_ids"] = [remaining["id"] for remaining in suite["cases"][index + 1:]]
                break
        if os.environ.get("PARITY_INJECT_FAILURE") == "test":
            raise RuntimeError("intentional test failure after diagnostic capture")
        statuses = [case["status"] for case in report["cases"]]
        report["status"] = "failed" if "failed" in statuses else "gaps" if "gap" in statuses else "passed"
    except BaseException as error:
        report.update(error=str(error), traceback=traceback.format_exc())
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2), flush=True)
    return {"passed": 0, "failed": 1, "gaps": 2}[report["status"]]


if __name__ == "__main__":
    raise SystemExit(main())
