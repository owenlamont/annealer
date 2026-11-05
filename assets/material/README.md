# Material Icon Theme file icons

Subset of [Material Icon Theme][upstream], under the included [MIT licence](LICENSE),
from commit [`f368264282ef14ee7feb012fd54e825181eff9c3`][commit]. The repository
licence covers the icons; no separate icon licence is present at the pinned commit.

The SVGs retain upstream artwork with trailing newlines normalised. The generic
`file.svg` reproduces upstream `generateFileIcons` in
`src/core/generator/fileGenerator.ts`, using the default `#90a4ae` from
`src/core/generator/config/defaultConfig.ts`; upstream generates rather than tracks
the generic SVG.

`mapping.json` contains the selected icons' effective associations from upstream
`src/core/icons/fileIcons.ts`, with upstream `parseByPattern` applied and default
icon-pack settings. Filename keys are lowercased. Special filenames precede
extensions; compound extensions match longest first. Unvendored kinds use the
generic file icon when no selected association matches.

Regenerate the PNG mip atlases and Rust table with:

```sh
uv run --script scripts/rasterise_icons.py
cargo fmt --all
```

CairoSVG needs the Cairo system library. The executable embeds the PNGs; Cairo and
an SVG renderer are unnecessary to build or run Annealer.

Each atlas is a transparent 256×512 PNG. A 256-pixel square base image and every
half-size mip down to 1×1 sit vertically, aligned left, with one unused bottom row.
Rasterisation preserves upstream colours and aspect ratio, trims outer whitespace
and adds an eight-pixel margin. Resizing uses premultiplied alpha. The renderer
packs the levels into one texture with linear mip filtering, preserving close-view
detail while reducing minification shimmer.

Neon uses untinted, unlit icons without backing tiles. Thin directory-coloured rings
show membership; larger temporary author-coloured rings show recent touches.
`Theme` owns art, tint, placement, ring geometry and optional tile presentation.

[upstream]: https://github.com/material-extensions/vscode-material-icon-theme
[commit]: https://github.com/material-extensions/vscode-material-icon-theme/tree/f368264282ef14ee7feb012fd54e825181eff9c3
