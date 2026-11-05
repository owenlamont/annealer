# /// script
# requires-python = ">=3.15"
# dependencies = ["pytest>=9"]
# ///

import subprocess
import sys
from pathlib import Path
from typing import Final

import pytest

_SCRIPT: Final = Path(__file__).resolve().parents[1] / "scripts/import_icon_mapping.py"


def test_help_does_not_download_a_bundle() -> None:
    result = subprocess.run(
        [sys.executable, _SCRIPT, "--help"],
        capture_output=True,
        text=True,
        timeout=10,
        check=True,
    )
    assert "--vsix" in result.stdout


def test_bad_hash_leaves_existing_assets_untouched(tmp_path: Path) -> None:
    bundle = tmp_path / "bad.vsix"
    bundle.write_bytes(b"not a bundle")
    output = tmp_path / "assets"
    output.mkdir()
    marker = output / "keep.svg"
    marker.write_bytes(b"existing")
    result = subprocess.run(
        [sys.executable, _SCRIPT, "--vsix", bundle, "--output", output],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    assert result.returncode != 0
    assert "SHA-256 mismatch" in result.stderr
    assert list(output.iterdir()) == [marker]
    assert marker.read_bytes() == b"existing"


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-W", "error"]))
