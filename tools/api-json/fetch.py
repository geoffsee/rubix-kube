"""Explicit preparation step; verify locked release artifact content before execution."""
import hashlib
import json
import pathlib
import platform
import urllib.request

architecture = {"aarch64": "arm64", "x86_64": "amd64"}[platform.machine()]
inputs = json.loads(pathlib.Path("/experiment/inputs.json").read_text())
for name, artifact in inputs["artifacts"][architecture].items():
    destination = pathlib.Path("/usr/local/bin") / name
    digest = hashlib.sha256()
    with urllib.request.urlopen(artifact["url"], timeout=120) as response:
        with destination.open("wb") as output:
            while chunk := response.read(1024 * 1024):
                digest.update(chunk)
                output.write(chunk)
    if digest.hexdigest() != artifact["sha256"]:
        destination.unlink()
        raise RuntimeError(f"checksum mismatch: {name}")
    destination.chmod(0o755)
    print(f"verified {name}: {digest.hexdigest()}", flush=True)
