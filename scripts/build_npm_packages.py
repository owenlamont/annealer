# /// script
# requires-python = ">=3.15"
# dependencies = []
# ///

import argparse
import json
import shutil
import zipfile
from pathlib import Path
from typing import Final

_ROOT: Final = Path(__file__).resolve().parents[1]
_LICENSES: Final = (
    "LICENSE",
    "LICENSE-APACHE",
    "assets/material/LICENSE.txt",
    "THIRDPARTY.txt",
)


def _write_json(path: Path, data: object) -> None:
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def _copy_assets(package_dir: Path) -> None:
    for name in (*_LICENSES, "README.md"):
        destination = package_dir / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(_ROOT / name, destination)


def _build(*, assets_dir: Path, out_dir: Path, local_test: bool) -> None:
    package = json.loads((_ROOT / "package.json").read_text(encoding="utf-8"))
    platforms = json.loads((_ROOT / "npm-platforms.json").read_text(encoding="utf-8"))
    out_dir.mkdir(parents=True, exist_ok=True)
    for platform in platforms["platforms"]:
        package_dir = out_dir / platform["folderName"]
        if package_dir.exists():
            shutil.rmtree(package_dir)
        (package_dir / "bin").mkdir(parents=True)
        _copy_assets(package_dir)
        with zipfile.ZipFile(assets_dir / platform["archiveName"]) as archive:
            (member,) = (
                item
                for item in archive.infolist()
                if not item.is_dir()
                and Path(item.filename).name == platform["binaryName"]
            )
            binary = package_dir / "bin" / platform["binaryName"]
            binary.write_bytes(archive.read(member))
        binary.chmod(0o755)
        metadata = {
            key: package[key]
            for key in (
                "version",
                "description",
                "author",
                "license",
                "homepage",
                "bugs",
                "repository",
            )
        }
        metadata.update(
            name=platform["packageName"],
            type="commonjs",
            files=["bin/", "README.md", *_LICENSES],
            os=platform["os"],
            cpu=platform["cpu"],
            publishConfig={"access": "public"},
            preferUnplugged=True,
        )
        if "libc" in platform:
            metadata["libc"] = platform["libc"]
        _write_json(package_dir / "package.json", metadata)
    meta_dir = out_dir / "codestrand"
    if meta_dir.exists():
        shutil.rmtree(meta_dir)
    (meta_dir / "bin").mkdir(parents=True)
    _copy_assets(meta_dir)
    shutil.copy2(_ROOT / "bin/codestrand.js", meta_dir / "bin/codestrand.js")
    (meta_dir / "bin/codestrand.js").chmod(0o755)
    shutil.copy2(_ROOT / "npm-platforms.json", meta_dir / "npm-platforms.json")
    package["optionalDependencies"] = {
        platform["packageName"]: (
            f"file:../{platform['folderName']}" if local_test else package["version"]
        )
        for platform in platforms["platforms"]
    }
    _write_json(meta_dir / "package.json", package)


def main() -> None:
    """Replace staged npm packages using binary zip artifacts."""
    parser = argparse.ArgumentParser(description="Build npm meta and platform packages")
    parser.add_argument("--assets-dir", required=True, type=Path)
    parser.add_argument("--out-dir", required=True, type=Path)
    parser.add_argument(
        "--local-test",
        action="store_true",
        help="Use file: optionalDependencies for local installation",
    )
    args = parser.parse_args()
    _build(assets_dir=args.assets_dir, out_dir=args.out_dir, local_test=args.local_test)


if __name__ == "__main__":
    main()
