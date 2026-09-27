"""Run repository-owned tooling regressions without hosted services."""
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
for directory in sorted({path.parent for path in (root / "tools").glob("*/test_*.py")}):
    subprocess.run(
        [sys.executable, "-m", "unittest", "discover", "-s", str(directory), "-p", "test_*.py"],
        cwd=root,
        check=True,
    )
