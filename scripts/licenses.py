# /// script
# requires-python = ">=3.15"
# dependencies = []
# ///

import argparse
import subprocess
import tempfile
from pathlib import Path
from typing import Final

_ROOT: Final = Path(__file__).resolve().parents[1]


def main() -> None:
    """Regenerate notices; --check exits nonzero when committed notices differ."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "THIRDPARTY.txt"
        subprocess.run(
            [
                "pixi",
                "exec",
                "--spec",
                "cargo-about=0.9.3",
                "cargo-about",
                "generate",
                "--locked",
                "--fail",
                "about.hbs",
                "--output-file",
                str(output),
            ],
            cwd=_ROOT,
            check=True,
            timeout=600,
        )
        generated = (
            "\n".join(
                line.rstrip()
                for line in output.read_text(encoding="utf-8").splitlines()
            ).rstrip()
            + "\n"
        )
    destination = _ROOT / "THIRDPARTY.txt"
    if args.check:
        if destination.read_text(encoding="utf-8") != generated:
            raise SystemExit("Stale notices: run uv run --script scripts/licenses.py")
    else:
        destination.write_text(generated, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
