"""Disposable E01.02 protocol experiment, not the production Rust supervisor."""
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import socket
import sqlite3
import ssl
import subprocess
import time
import traceback
import urllib.error
import urllib.request

STATE = Path("/state")
EVIDENCE = Path("/evidence")
children = []
handles = []
report = {"status": "failed", "checks": [], "measurements": [], "shutdowns": []}


def check(condition, message):
    if not condition:
        raise RuntimeError(message)
    report["checks"].append(message)


def openssl(*args):
    subprocess.run(["openssl", *args], check=True, stdout=subprocess.DEVNULL,
                   stderr=subprocess.PIPE, cwd=STATE)


def credentials():
    os.umask(0o077)
    openssl("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
            "-subj", "/CN=boundary-test-ca", "-keyout", "ca.key", "-out", "ca.crt",
            "-addext", "basicConstraints=critical,CA:TRUE",
            "-addext", "keyUsage=critical,keyCertSign,cRLSign")
    for name, subject, extensions in [
        ("server", "/CN=localhost", "subjectAltName=IP:127.0.0.1,DNS:localhost\nextendedKeyUsage=serverAuth\n"),
        ("admin", "/CN=boundary-admin/O=system:masters", "extendedKeyUsage=clientAuth\n"),
        ("unprivileged", "/CN=boundary-unprivileged", "extendedKeyUsage=clientAuth\n"),
    ]:
        openssl("req", "-newkey", "rsa:2048", "-nodes", "-subj", subject,
                "-keyout", f"{name}.key", "-out", f"{name}.csr")
        (STATE / f"{name}.ext").write_text(
            "basicConstraints=critical,CA:FALSE\n"
            "keyUsage=critical,digitalSignature,keyEncipherment\n" + extensions)
        openssl("x509", "-req", "-in", f"{name}.csr", "-CA", "ca.crt",
                "-CAkey", "ca.key", "-CAcreateserial", "-days", "1",
                "-extfile", f"{name}.ext", "-out", f"{name}.crt")
    openssl("genrsa", "-out", "service-account.key", "2048")


def context(identity):
    result = ssl.create_default_context(cafile=str(STATE / "ca.crt"))
    if identity:
        result.load_cert_chain(STATE / f"{identity}.crt", STATE / f"{identity}.key")
    return result


def request(method, path, body=None, identity="admin"):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request("https://127.0.0.1:6443" + path, data=data,
                                 method=method, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, context=context(identity), timeout=5) as response:
            raw = response.read()
            return response.status, json.loads(raw) if raw.startswith(b"{") else raw.decode()
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode()


def start(name, args, cycle):
    log = (EVIDENCE / f"{name}-{cycle}.log").open("w")
    handles.append(log)
    process = subprocess.Popen([name, *args], stdout=log, stderr=subprocess.STDOUT,
                               cwd=STATE, start_new_session=True)
    children.append((name, process))
    return process


def process_alive():
    for name, process in children:
        if process.poll() is not None:
            raise RuntimeError(f"{name} exited early with {process.returncode}")


def start_kine(cycle):
    start("kine", ["--listen-address=127.0.0.1:2379",
                   "--endpoint=sqlite:///state/state.db?_journal_mode=WAL&_busy_timeout=30000"
                   "&_synchronous=NORMAL&_txlock=immediate&_stmt_cache_size=20&cache=shared",
                   "--metrics-bind-address=0",
                   "--datastore-max-idle-connections=3",
                   "--datastore-max-open-connections=5",
                   "--datastore-connection-max-lifetime=60s",
                   "--watch-progress-notify-interval=15s"], cycle)
    deadline = time.monotonic() + 30
    while True:
        process_alive()
        try:
            with socket.create_connection(("127.0.0.1", 2379), timeout=1):
                break
        except OSError:
            if time.monotonic() > deadline:
                raise RuntimeError("Kine socket startup timeout")
            time.sleep(0.2)


def ready(cycle):
    began = time.monotonic()
    start_kine(cycle)
    start("kube-apiserver", [
        "--bind-address=127.0.0.1", "--advertise-address=192.0.2.1", "--secure-port=6443",
        "--etcd-servers=http://127.0.0.1:2379", "--service-cluster-ip-range=10.43.0.0/16",
        "--tls-cert-file=/state/server.crt", "--tls-private-key-file=/state/server.key",
        "--client-ca-file=/state/ca.crt", "--anonymous-auth=false", "--authorization-mode=Node,RBAC",
        "--service-account-issuer=https://kubernetes.default.svc",
        "--service-account-signing-key-file=/state/service-account.key",
        "--service-account-key-file=/state/service-account.key",
        "--profiling=false", "--shutdown-delay-duration=0s",
    ], cycle)
    wait_ready(cycle, began)


def wait_ready(cycle, began):
    deadline = time.monotonic() + 120
    last_readiness = "not requested"
    while time.monotonic() < deadline:
        process_alive()
        try:
            status, body = request("GET", "/readyz?verbose")
            last_readiness = {"http_status": status, "body": body}
            if status == 200:
                measure(cycle, time.monotonic() - began)
                return
        except (OSError, urllib.error.URLError) as error:
            last_readiness = str(error)
        time.sleep(0.5)
    raise RuntimeError(f"API readiness timeout: {last_readiness}")


def measure(cycle, elapsed):
    processes = []
    for name, process in children:
        values = {}
        for line in Path(f"/proc/{process.pid}/status").read_text().splitlines():
            if line.startswith(("VmRSS:", "VmHWM:")):
                key, value, _ = line.split()
                values[key.rstrip(":") + "_kib"] = int(value)
        processes.append({"name": name, "pid": process.pid, **values})
    report["measurements"].append({"cycle": cycle, "readiness_seconds": elapsed,
                                    "component_process_count": len(processes),
                                    "components": processes,
                                    "component_rss_sum_kib": sum(p["VmRSS_kib"] for p in processes)})


def shutdown():
    failures = []
    # Dependency order, independent of spawn order after a component restart.
    for name, process in sorted(children, key=lambda child: child[0] != "kube-apiserver"):
        forced = False
        group_alive = False
        error = None
        try:
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    forced = True
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    process.wait(timeout=5)
            try:
                os.killpg(process.pid, 0)
                group_alive = True
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            except ProcessLookupError:
                pass
        except (OSError, subprocess.SubprocessError) as failure:
            error = str(failure)
        report["shutdowns"].append({"component": name, "pid": process.pid,
                                    "exit_code": process.returncode, "forced": forced,
                                    "owned_group_remained": group_alive, "error": error})
        if error or forced or group_alive or process.returncode not in (0, -signal.SIGTERM):
            failures.append(name)
    children.clear()
    for log in handles:
        try:
            log.close()
        except OSError as failure:
            failures.append(str(failure))
    handles.clear()
    if failures:
        raise RuntimeError(f"unclean owned-process shutdown: {failures}")


def run():
    architecture = {"aarch64": "arm64", "x86_64": "amd64"}[platform.machine()]
    inputs = json.loads(Path("/experiment/inputs.json").read_text())
    report.update({"inputs": inputs, "architecture": architecture,
                   "kernel": platform.release(), "started_at_unix": time.time(),
                   "fixture": "unprivileged isolated Docker container; no runtime or workloads",
                   "spike_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
    for name, artifact in inputs["artifacts"][architecture].items():
        content = Path("/usr/local/bin") / name
        digest = hashlib.sha256(content.read_bytes()).hexdigest()
        check(digest == artifact["sha256"], f"locked {name} digest verified at runtime")
    credentials()
    ready(1)
    collection = "/api/v1/namespaces/default/configmaps"
    item = collection + "/boundary-probe"
    check(request("GET", collection, identity=None)[0] == 401, "unauthenticated request rejected")
    check(request("GET", collection, identity="unprivileged")[0] == 403,
          "authenticated identity without RBAC permission rejected")
    body = {"apiVersion": "v1", "kind": "ConfigMap", "metadata": {"name": "boundary-probe"},
            "data": {"value": "created"}}
    status, created = request("POST", collection, body)
    check(status == 201, "authenticated create succeeded")
    status, observed = request("GET", item)
    check(status == 200 and observed["data"]["value"] == "created", "authenticated read succeeded")
    observed["data"]["value"] = "updated"
    status, updated = request("PUT", item, observed)
    check(status == 200 and updated["data"]["value"] == "updated", "authenticated update succeeded")
    uid = created["metadata"]["uid"]
    shutdown()
    check((STATE / "state.db").is_file(), "SQLite database persisted after shutdown")
    with sqlite3.connect(f"file:{STATE / 'state.db'}?mode=ro", uri=True) as database:
        check(database.execute("PRAGMA integrity_check").fetchone()[0] == "ok", "SQLite integrity check passed")
        count = database.execute("SELECT count(*) FROM kine WHERE name=?",
                                 ("/registry/configmaps/default/boundary-probe",)).fetchone()[0]
        check(count > 0, "independent SQLite query found persisted API object")
    ready(2)
    status, recovered = request("GET", item)
    check(status == 200 and recovered["data"]["value"] == "updated"
          and recovered["metadata"]["uid"] == uid, "same updated object survived component restart")
    recovered["data"]["value"] = "before-abrupt-kill"
    status, _ = request("PUT", item, recovered)
    check(status == 200, "update acknowledged before abrupt datastore termination")
    for index, (name, process) in enumerate(children):
        if name == "kine":
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)
            check(process.returncode == -signal.SIGKILL, "intentional datastore SIGKILL observed")
            group_alive = False
            try:
                os.killpg(process.pid, 0)
                group_alive = True
            except ProcessLookupError:
                pass
            report["shutdowns"].append({"component": name, "pid": process.pid,
                                        "exit_code": process.returncode, "injected_failure": "SIGKILL",
                                        "owned_group_remained": group_alive})
            # Keep ownership tracked on failure so final cleanup can kill remaining descendants.
            check(not group_alive, "fault-injected datastore process group is absent")
            children.pop(index)
            break
    else:
        raise RuntimeError("datastore process missing before fault injection")
    api_pid = next(process.pid for name, process in children if name == "kube-apiserver")
    outage_began = time.monotonic()
    deadline = outage_began + 30
    while time.monotonic() < deadline:
        process_alive()
        try:
            status, body = request("GET", "/readyz?verbose")
            if status != 200:
                report["datastore_outage"] = {"http_status": status, "body": body}
                break
        except (OSError, urllib.error.URLError) as error:
            report["datastore_outage"] = {"request_error": str(error)}
            break
        time.sleep(0.5)
    else:
        raise RuntimeError("API readiness did not reflect datastore outage")
    start_kine("recovery")
    wait_ready("datastore-recovery", outage_began)
    check(next(process.pid for name, process in children if name == "kube-apiserver") == api_pid,
          "existing API process recovered after datastore-only restart")
    status, recovered = request("GET", item)
    check(status == 200 and recovered["data"]["value"] == "before-abrupt-kill"
          and recovered["metadata"]["uid"] == uid, "acknowledged update survived abrupt datastore restart")
    check(request("DELETE", item)[0] == 200, "authenticated delete succeeded")
    check(request("GET", item)[0] == 404, "deleted object is absent")
    shutdown()
    ready(4)
    check(request("GET", item)[0] == 404, "deletion survived second component restart")
    shutdown()
    check(not children, "all owned component processes reaped")
    report["status"] = "passed"


try:
    run()
except BaseException as error:
    report["error"] = str(error)
    report["traceback"] = traceback.format_exc()
finally:
    try:
        shutdown()
    except BaseException as error:
        report["status"] = "failed"
        report["cleanup_error"] = str(error)
    report["finished_at_unix"] = time.time()
    (EVIDENCE / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2), flush=True)
raise SystemExit(0 if report["status"] == "passed" else 1)
