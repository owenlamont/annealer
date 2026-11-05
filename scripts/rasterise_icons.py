# /// script
# requires-python = ">=3.15"
# dependencies = ["cairosvg==2.9.1", "pillow==13.0.0"]
# ///

import io
from pathlib import Path
from typing import Final

import cairosvg
from PIL import Image, ImageOps

_ASSETS: Final = Path(__file__).resolve().parents[1] / "assets" / "seti"
_SIZE: Final = 48
_PADDING: Final = 2


def _rasterise(source: Path) -> None:
    rendered = cairosvg.svg2png(url=str(source), output_width=512, output_height=512)
    with Image.open(io.BytesIO(rendered)) as image:
        alpha = image.convert("RGBA").getchannel("A")
        cropped = alpha.crop(alpha.getbbox())
        fitted = ImageOps.contain(
            cropped,
            (_SIZE - 2 * _PADDING, _SIZE - 2 * _PADDING),
            method=Image.Resampling.LANCZOS,
        )
    mask = Image.new("L", (_SIZE, _SIZE))
    mask.paste(fitted, ((_SIZE - fitted.width) // 2, (_SIZE - fitted.height) // 2))
    glyph = Image.new("RGBA", (_SIZE, _SIZE), "white")
    glyph.putalpha(mask)
    glyph.save(_ASSETS / "png" / f"{source.stem}.png")


if __name__ == "__main__":
    for svg in sorted((_ASSETS / "svg").glob("*.svg")):
        _rasterise(svg)
