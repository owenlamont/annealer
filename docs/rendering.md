# Rendering reference

The feasibility probe uses **Bevy 0.20.1**, with an exact dependency requirement and a
committed Cargo lockfile. Stay near current stable releases; upgrade deliberately in a
feature branch. Read the dependency versions before writing Bevy code.

## API sources

Use the matching release tag and versioned rustdoc. Upstream main and unversioned
tutorials may describe incompatible APIs.

- [Externally driven headless rendering][headless]: target image, manual updates,
  synchronous pipeline compilation and GPU polling.
- [Screenshot implementation][screenshots]: `ScreenshotCaptured` carries the image;
  the built-in saver logs write errors rather than propagating them.
- [Basic 3D scene][basic] and [transparency][transparency]: `Camera3d`, `Mesh3d`,
  `MeshMaterial3d`, `StandardMaterial` and `AlphaMode`.
- [Camera-targeted UI][ui]: `UiTargetCamera`, `Text`, `TextFont::from_font_size` and
  absolute UI nodes. Probe labels are projected UI text rather than world-space meshes.
- [Bevy 0.20.1 API][api] for signatures and types absent from those examples.

## Local patterns

The executable is the compile-checked recipe; avoid copying snippets into another
manual. [scene.rs](../src/scene.rs) builds the shared scene, and
[export.rs](../src/export.rs) drives capture.

- `model::sample(frame, fps)` determines animation state without wall-clock access.
  Preview converts elapsed time to a frame; export holds the selected frame throughout
  any extra updates required by readback.
- Export disables the window runner and pipelined rendering, waits for plugins, and
  calls `finish` and `cleanup` before manual updates.
- The image is the camera's render target. UI explicitly targets that camera too.
- Embedded default-font glyph layout and queued render pipelines must be ready before
  capture begins. The probe loads no external scene assets.
- Each capture has a dedicated channel. Export advances only after receiving the image
  and successfully writing the PNG. Readiness, readback and GPU polling have deadlines.
- Tonemapping and dithering are disabled for the probe's unlit palette. No custom shader
  or third-party Bevy plugin is required.

Controlled logical time does not guarantee identical pixels across GPU backends.
Bevy's transitive platform dependencies require multiple crate versions; the crate's
Clippy expectation records that constraint without relaxing application-code lints.

## Check a rendering change

```sh
prek run --all-files
cargo test --locked --all-targets
cargo run --locked -- --help
cargo run --locked -- preview
cargo test --locked --test render_export -- --ignored
```

The graphics test exports two sequences with different pacing, verifies PNG count,
numbering and dimensions, compares decoded images and logical state, and checks that
the scene advances. Avoid treating a same-backend result as a cross-platform pixel
guarantee. Inspect early, middle and final frames for readable labels, creation/deletion,
upward and downward fragments, simultaneous beams and changing overlap between colours.

The probe intentionally includes overlapping cyan and amber beams. An upstream
[transparent-mesh sorting report][sorting] affects Bevy 0.20; inspect the intended
shots before choosing a transparency treatment for the full study. The probe does not
claim to fix or comprehensively reproduce that report.

## Upgrade Bevy

1. Check the latest stable release metadata and update the dependency and lockfile.
2. Read the relevant [official migration guide][migration], then refresh the tagged
   source links used by the affected code.
3. Compile and run the checks above, including preview and export. Inspect transparency
   and labels even when compilation passes.
4. Update the version and any changed local patterns in this reference.

The 0.19 to 0.20 guide covers WESL shaders, opt-in screen-space transmission, sprite
ordering changes and weak schedule ordering. Read the actual section when touching
those APIs; a list of historic renamed methods is not a reliable coding reference.

[headless]: https://github.com/bevyengine/bevy/blob/v0.20.1/examples/app/externally_driven_headless_renderer.rs
[screenshots]: https://github.com/bevyengine/bevy/blob/v0.20.1/crates/bevy_render/src/view/window/screenshot.rs
[basic]: https://github.com/bevyengine/bevy/blob/v0.20.1/examples/3d/3d_scene.rs
[transparency]: https://github.com/bevyengine/bevy/blob/v0.20.1/examples/3d/transparency_3d.rs
[ui]: https://github.com/bevyengine/bevy/blob/v0.20.1/examples/ui/ui_target_camera.rs
[api]: https://docs.rs/bevy/0.20.1/bevy/
[migration]: https://bevy.org/learn/migration-guides/0-19-to-0-20/
[sorting]: https://github.com/bevyengine/bevy/issues/26390
