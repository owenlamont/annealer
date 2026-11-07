# /// script
# requires-python = ">=3.15"
# dependencies = ["pytest>=9"]
# ///

import json
import os
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path
from typing import Final

import pytest

_ROOT: Final = Path(__file__).resolve().parents[1]
_PLATFORMS: Final = json.loads((_ROOT / "npm-platforms.json").read_text())["platforms"]
_LICENSES: Final = (
    "LICENSE",
    "LICENSE-APACHE",
    "assets/material/LICENSE.txt",
    "THIRDPARTY.txt",
)


@pytest.fixture
def assets(tmp_path: Path) -> Path:
    directory = tmp_path / "assets"
    directory.mkdir()
    for platform in _PLATFORMS:
        with zipfile.ZipFile(directory / platform["archiveName"], "w") as archive:
            archive.writestr(platform["binaryName"], b"fake executable")
    return directory


def _build(*, assets: Path, output: Path, arguments: tuple[str, ...] = ()) -> None:
    subprocess.run(
        [
            sys.executable,
            _ROOT / "scripts/build_npm_packages.py",
            "--assets-dir",
            assets,
            "--out-dir",
            output,
            *arguments,
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )


@pytest.mark.parametrize("platform", _PLATFORMS, ids=lambda p: p["folderName"])
def test_platform_package(
    assets: Path, tmp_path: Path, platform: dict[str, object]
) -> None:
    output = tmp_path / "packages"
    _build(assets=assets, output=output)
    package_dir = output / str(platform["folderName"])
    metadata = json.loads((package_dir / "package.json").read_text())
    assert metadata["name"] == platform["packageName"]
    assert (
        metadata["version"]
        == json.loads((_ROOT / "package.json").read_text())["version"]
    )
    assert metadata["os"] == platform["os"]
    assert metadata["cpu"] == platform["cpu"]
    assert metadata.get("libc") == platform.get("libc")
    assert (
        package_dir / "bin" / str(platform["binaryName"])
    ).read_bytes() == b"fake executable"
    assert {name: (package_dir / name).read_bytes() for name in _LICENSES} == {
        name: (_ROOT / name).read_bytes() for name in _LICENSES
    }


@pytest.mark.parametrize("local", [False, True])
def test_meta_package(assets: Path, tmp_path: Path, *, local: bool) -> None:
    output = tmp_path / "packages"
    _build(assets=assets, output=output, arguments=("--local-test",) if local else ())
    package_dir = output / "codestrand"
    metadata = json.loads((package_dir / "package.json").read_text())
    assert metadata["optionalDependencies"] == {
        p["packageName"]: f"file:../{p['folderName']}" if local else metadata["version"]
        for p in _PLATFORMS
    }
    assert (package_dir / "bin/codestrand.js").read_bytes() == (
        _ROOT / "bin/codestrand.js"
    ).read_bytes()
    assert {name: (package_dir / name).read_bytes() for name in _LICENSES} == {
        name: (_ROOT / name).read_bytes() for name in _LICENSES
    }


@pytest.mark.parametrize(
    "members", [(), ("wrong-codestrand",), ("codestrand", "nested/codestrand")]
)
def test_invalid_archive(
    assets: Path, tmp_path: Path, members: tuple[str, ...]
) -> None:
    platform = next(p for p in _PLATFORMS if p["binaryName"] == "codestrand")
    with zipfile.ZipFile(assets / platform["archiveName"], "w") as archive:
        for name in members:
            archive.writestr(name, b"fake")
    with pytest.raises(subprocess.CalledProcessError):
        _build(assets=assets, output=tmp_path / "packages")


def test_missing_archive(tmp_path: Path) -> None:
    with pytest.raises(subprocess.CalledProcessError):
        _build(assets=tmp_path, output=tmp_path / "packages")


@pytest.mark.skipif(shutil.which("node") is None, reason="Node is not installed")
def test_launcher_forwards_arguments_and_exit_status() -> None:
    result = subprocess.run(
        [
            "node",
            _ROOT / "bin/codestrand.js",
            "-c",
            "import sys; print(sys.argv[1]); sys.exit(7)",
            "argument with spaces",
        ],
        env=os.environ | {"CODESTRAND_BINARY_PATH": sys.executable},
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    assert result.returncode == 7
    assert result.stdout.strip() == "argument with spaces"


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-W", "error"]))
