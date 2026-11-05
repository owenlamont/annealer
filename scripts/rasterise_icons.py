# /// script
# requires-python = ">=3.15"
# dependencies = ["cairosvg==2.9.1", "pillow==13.0.0"]
# ///

import io
import json
from pathlib import Path
from typing import Final, TypedDict

import cairosvg
from PIL import Image, ImageOps

_ROOT: Final = Path(__file__).resolve().parents[1]
_ASSETS: Final = _ROOT / "assets" / "material"
_SIZE: Final = 256
_PADDING: Final = 8


class _Glyph(TypedDict):
    name: str
    filenames: list[str]
    extensions: list[str]


def _rasterise(source: Path) -> None:
    rendered = cairosvg.svg2png(url=str(source), output_width=512, output_height=512)
    with Image.open(io.BytesIO(rendered)) as image:
        rgba = image.convert("RGBA")
        cropped = rgba.crop(rgba.getchannel("A").getbbox())
        fitted = ImageOps.contain(
            cropped.convert("RGBa"),
            (_SIZE - 2 * _PADDING, _SIZE - 2 * _PADDING),
            method=Image.Resampling.LANCZOS,
        )
    glyph = Image.new("RGBa", (_SIZE, _SIZE))
    glyph.paste(fitted, ((_SIZE - fitted.width) // 2, (_SIZE - fitted.height) // 2))
    atlas = Image.new("RGBA", (_SIZE, _SIZE * 2))
    offset = 0
    size = _SIZE
    while size:
        mip = glyph.resize((size, size), Image.Resampling.LANCZOS).convert("RGBA")
        atlas.paste(mip, (0, offset))
        offset += size
        size //= 2
    atlas.save(_ASSETS / "png" / f"{source.stem}.png")


def _rust_table(glyphs: list[_Glyph]) -> str:
    entries = []
    for glyph in glyphs:
        icon = glyph["name"]
        filenames = ", ".join(json.dumps(name) for name in glyph["filenames"])
        extensions = ", ".join(json.dumps(ext) for ext in glyph["extensions"])
        entries.append(
            "    super::FileGlyph {\n"
            f'        png: include_bytes!("../../assets/material/png/{icon}.png"),\n'
            f"        filenames: &[{filenames}],\n"
            f"        extensions: &[{extensions}],\n"
            "    },"
        )
    return (
        "pub(super) const GLYPHS: &[super::FileGlyph] = &[\n"
        + "\n".join(entries)
        + "\n];\n"
    )


if __name__ == "__main__":
    mappings: list[_Glyph] = json.loads((_ASSETS / "mapping.json").read_text())
    for mapping in mappings:
        _rasterise(_ASSETS / "svg" / f"{mapping['name']}.svg")
    (_ROOT / "src" / "theme" / "material.rs").write_text(_rust_table(mappings))
