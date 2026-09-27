#!/usr/bin/env python3
"""Build and run the fixture; copy evidence without exposing host paths to it."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, help="new directory for evidence (must not exist)")
args = parser.parse_args()
source = Path(__file__).resolve().parent
output = args.output or Path(tempfile.gettempdir()) / f"rubix-boundary-evidence-{uuid.uuid4().hex}"
output.mkdir(parents=True, exist_ok=False)
image = f"rubix-boundary-spike:{uuid.uuid4().hex}"
# Name ownership before creation so a timed-out create can still be cleaned up.
container = f"rubix-boundary-spike-{uuid.uuid4().hex}"
status = 1
errors = []
source_hashes = {name: hashlib.sha256((source / name).read_bytes()).hexdigest()
                 for name in ("Dockerfile", ".dockerignore", "inputs.json", "fetch.py", "spike.py", "run.py")}


def attempt(label, command, destination=None):
    """Collect independent diagnostics and cleanup errors without masking the first failure."""
    try:
        result = subprocess.run(command, check=True, timeout=30,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if destination:
            destination.write_text(result.stdout)
        return result.stdout
    except (OSError, subprocess.SubprocessError) as error:
        errors.append({"operation": label, "error": str(error),
                       "stderr": str(getattr(error, "stderr", ""))})
        return None


try:
    subprocess.run(["docker", "build", "--tag", image, str(source)], check=True, timeout=900)
    subprocess.run([
        "docker", "create", "--name", container, "--network=none", "--cap-drop=ALL",
        "--security-opt=no-new-privileges", "--memory=1g", "--pids-limit=256", image,
    ], check=True, timeout=30)
    try:
        status = subprocess.run(["docker", "start", "--attach", container], timeout=600).returncode
    except subprocess.TimeoutExpired:
        attempt("kill timed-out container", ["docker", "kill", container])
        status = 124
    finally:
        attempt("copy evidence", ["docker", "cp", f"{container}:/evidence/.", str(output)])
        attempt("inspect container", ["docker", "inspect", container], output / "container-inspect.json")
        attempt("inspect image", ["docker", "image", "inspect", image], output / "image-inspect.json")
except (OSError, subprocess.SubprocessError) as error:
    errors.append({"operation": "build or execute", "error": str(error)})
finally:
    # Inventory first distinguishes a legitimately absent target after a failed build
    # from a Docker daemon failure. Failed inventory is itself a cleanup failure.
    targets = attempt("inventory owned container", ["docker", "container", "ls", "--all",
                      "--filter", f"name=^/{container}$", "--format", "{{.Names}}"])
    if targets and container in targets.splitlines():
        attempt("remove owned container", ["docker", "rm", "--force", container])
    remaining = attempt("verify container removal", ["docker", "container", "ls", "--all",
                        "--filter", f"name=^/{container}$", "--format", "{{.Names}}"])
    if remaining and container in remaining.splitlines():
        errors.append({"operation": "verify container removal", "error": "owned container remains"})
    images = attempt("inventory owned image", ["docker", "image", "ls", "--quiet", image])
    if images and images.strip():
        attempt("remove owned image", ["docker", "image", "rm", image])
    remaining_images = attempt("verify image removal", ["docker", "image", "ls", "--quiet", image])
    if remaining_images and remaining_images.strip():
        errors.append({"operation": "verify image removal", "error": "owned image remains"})
    if errors:
        status = status or 1
    (output / "runner-result.json").write_text(json.dumps({
        "exit_code": status, "errors": errors, "owned_container": container, "owned_image": image,
        "source_sha256": source_hashes,
    }, indent=2) + "\n")
    print(f"Evidence: {output}", file=sys.stderr)
raise SystemExit(status)
