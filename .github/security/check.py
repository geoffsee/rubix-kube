"""Run local scanners; scanner errors, skipped Rust files and findings fail CI."""

import json
import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[2]
RULES = ROOT / ".github/security/rules"


def main():
    os.chdir(ROOT)
    env = dict(os.environ)
    for name in ("SEMGREP_APP_TOKEN", "GH_TOKEN", "GITHUB_TOKEN", "ZIZMOR_GITHUB_TOKEN"):
        env.pop(name, None)
    env.update(SEMGREP_SEND_METRICS="off", SEMGREP_ENABLE_VERSION_CHECK="0",
               ZIZMOR_OFFLINE="1")
    semgrep = ["semgrep", "scan", "--metrics=off", "--disable-version-check"]
    subprocess.run(semgrep + ["--test", str(RULES)], env=env, check=True)

    tracked = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--", "*.rs"]
    ).decode().split("\0")
    sources = sorted({p for p in tracked if p and Path(p).is_file()
                      and not p.startswith(".github/security/rules/")})
    if not sources:
        raise SystemExit("No Rust source files found; refusing an empty security scan.")
    with tempfile.TemporaryDirectory(prefix="rubix-security-") as directory:
        report = Path(directory) / "semgrep.json"
        subprocess.run(semgrep + ["--config", str(RULES), "--strict", "--error",
                                 "--disable-nosem", "--no-git-ignore", "--json-output", str(report),
                                 "--", *sources], env=env, check=True)
        result = json.loads(report.read_text())
        scanned = {Path(p).resolve() for p in result["paths"]["scanned"]}
        missing = {str(Path(p)) for p in sources if Path(p).resolve() not in scanned}
        if missing or result.get("errors"):
            raise SystemExit(f"Incomplete Rust scan: missing={sorted(missing)}, "
                             f"errors={result.get('errors')}")
    subprocess.run(["zizmor", "--offline", "--format", "plain", ".github/workflows",
                    ".github/actions"], env=env, check=True)
    print(f"Security checks passed: {len(sources)} Rust files and local Actions definitions.")


if __name__ == "__main__":
    main()
