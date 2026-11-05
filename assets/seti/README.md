# Seti file glyphs

Subset of [Seti UI](https://github.com/jesseweed/seti-ui), under the included
[MIT licence](LICENSE), from commit
[`2d6c5e68b4ded73c92dac291845ee44e1182d511`](https://github.com/jesseweed/seti-ui/tree/2d6c5e68b4ded73c92dac291845ee44e1182d511).
The SVGs in `svg/` retain the upstream artwork with trailing newlines normalised.

Regenerate the committed white, transparent 48-pixel PNGs with:

```sh
uv run --script scripts/rasterise_icons.py
```

CairoSVG needs the Cairo system library. Rasterisation trims transparent padding,
preserves aspect ratio and adds a two-pixel border. The executable embeds the PNGs;
neither Cairo nor an SVG renderer is needed to build or run Annealer.

`Theme` owns the glyph art, tint, placement and filename/extension mapping. Exact
filenames take precedence; extensions match without ASCII case sensitivity. Unknown
files use the generic glyph beside the existing colour spine.
