# /// script
# requires-python = ">=3.15"
# dependencies = []
# ///

import argparse
import hashlib
import io
import json
import posixpath
import urllib.request
import zipfile
from pathlib import Path
from typing import Final, TypedDict

_VERSION: Final = "5.40.0"
_URL: Final = (
    "https://open-vsx.org/api/PKief/material-icon-theme/5.40.0/file/"
    "PKief.material-icon-theme-5.40.0.vsix"
)
_SHA256: Final = "55b565f200881abfd807baf9be52fce45fd52fef013ba71b0f8394984ce1ae06"
_OUTPUT: Final = Path(__file__).resolve().parents[1] / "assets" / "material"
_SELECTED: Final = (
    "file",
    "html",
    "markdown",
    "css",
    "json",
    "yaml",
    "toml",
    "javascript",
    "react",
    "react_ts",
    "typescript",
    "c",
    "h",
    "cpp",
    "hpp",
    "go",
    "go-mod",
    "python",
    "python-misc",
    "ruff",
    "uv",
    "console",
    "powershell",
    "license",
    "docker",
    "rust",
    "git",
    "lock",
    "nodejs",
    "npm",
    "yarn",
    "contributing",
    "readme",
    "changelog",
    "editorconfig",
    "pnpm",
)


class BundleError(Exception):
    pass


class _IconDefinition(TypedDict):
    iconPath: str


class _Manifest(TypedDict):
    file: str
    iconDefinitions: dict[str, _IconDefinition]
    fileExtensions: dict[str, str]
    fileNames: dict[str, str]


class _Association(TypedDict):
    name: str
    extensions: list[str]
    filenames: list[str]


def _import_bundle(data: bytes) -> dict[str, bytes]:
    if hashlib.sha256(data).hexdigest() != _SHA256:
        raise BundleError("Material Icon Theme VSIX SHA-256 mismatch")
    with zipfile.ZipFile(io.BytesIO(data)) as bundle:
        manifest: _Manifest = json.loads(
            bundle.read("extension/dist/material-icons.json")
        )
        if manifest["file"] != _SELECTED[0]:
            raise BundleError("Material Icon Theme fallback changed")
        files = {"LICENSE.txt": bundle.read("extension/LICENSE.txt")}
        mappings: list[_Association] = []
        for name in _SELECTED:
            icon = manifest["iconDefinitions"][name]
            source = posixpath.normpath("extension/dist/" + icon["iconPath"])
            files[f"svg/{name}.svg"] = bundle.read(source).rstrip(b"\r\n") + b"\n"
            mappings.append(
                {
                    "name": name,
                    "extensions": sorted(
                        key.lower()
                        for key, value in manifest["fileExtensions"].items()
                        if value == name
                    ),
                    "filenames": sorted(
                        key.lower()
                        for key, value in manifest["fileNames"].items()
                        if value == name
                    ),
                }
            )
        files["mapping.json"] = (
            json.dumps(mappings, indent=2, sort_keys=True, ensure_ascii=True) + "\n"
        ).encode()
    return files


def _main() -> None:
    parser = argparse.ArgumentParser(
        description="Vendor pinned Material Icon Theme SVGs"
    )
    parser.add_argument(
        "--vsix", type=Path, help="Use a cached VSIX (same pinned hash)"
    )
    parser.add_argument("--output", type=Path, default=_OUTPUT)
    args = parser.parse_args()
    if args.vsix:
        data = args.vsix.read_bytes()
    else:
        with urllib.request.urlopen(_URL, timeout=60) as response:
            data = response.read()
    files = _import_bundle(data)
    for name, contents in files.items():
        destination = args.output / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(contents)
    print(f"Vendored {len(_SELECTED)} Material Icon Theme {_VERSION} icons")


if __name__ == "__main__":
    _main()
