# Material Icon Theme file icons

Subset of [Material Icon Theme][upstream], from the pinned [Open VSX 5.40.0 VSIX][vsix].
The included [MIT licence](LICENSE.txt) comes from `extension/LICENSE.txt`.

VSIX SHA-256:

```text
55b565f200881abfd807baf9be52fce45fd52fef013ba71b0f8394984ce1ae06
```

Re-vendor with the stdlib-only Python script:

```sh
uv run --script scripts/import_icon_mapping.py
```

The script verifies the archive hash before parsing or writing assets. It reads the
expanded `extension/dist/material-icons.json`, copies the selected SVGs and licence,
normalises SVG final newlines, and writes trimmed `mapping.json` associations.
`--vsix FILE` accepts a cached copy with the same pinned hash; `--output DIRECTORY`
writes a separate copy for reproducibility checks.

Filename associations precede extensions; compound extensions match longest first,
without ASCII case sensitivity. Unmapped files use the manifest's generic file icon.
To change the subset, update the importer selection and embedded SVG registry in
`src/theme/icons.rs` together.

The executable embeds SVG sources and mapping JSON. The mapping is parsed once;
`resvg` 0.49 rasterises each icon during scene startup, with optional features
including text, system fonts and raster-image decoding disabled. No filesystem asset
lookup, Node, Cairo or pre-generated raster files are needed.

`Theme::glyph_texture` controls resolution, padding and mipmaps. Neon renders at
256 pixels with a 1/32 margin, fitting a raster probe's visible alpha bounds without
changing aspect ratio. Each mip level is rendered directly from the parsed SVG. Premultiplied
raster output is converted to straight-alpha sRGB for Bevy's blending and linear
mip filtering.

Neon keeps untinted icons flat on the floor, without backing tiles. Thin directory
rings show membership; larger temporary author rings show recent touches. `Theme`
owns the artwork, material, texture settings, transforms and optional tile style.

[upstream]: https://github.com/material-extensions/vscode-material-icon-theme
[vsix]: https://open-vsx.org/api/PKief/material-icon-theme/5.40.0/file/PKief.material-icon-theme-5.40.0.vsix
