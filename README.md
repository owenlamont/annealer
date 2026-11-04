# Annealer

![Annealer logo](assets/annealer.svg)

A Git history visualiser exploring how software takes shape over time.

Annealer aims to retain Gource's moving developers and growing directory graph,
adding restrained 3D depth and animated diff fragments. The annealing metaphor is
a structure gradually settling as changes reshape the repository.

**Status: rendering feasibility probe.** A synthetic scene exercises document-like
file tiles, two moving contributors, translucent connections, labels and diff
fragments. Preview and numbered PNG export share the same scene updates. Git import
and video encoding are not implemented. The repository remains private; licensing
and distribution are deferred, and Cargo publishing is disabled.

## Visual goal

- A mostly planar graph of repositories, directories and persistent file objects.
- Gentle layout relaxation as files appear, disappear or move, preserving spatial
  continuity instead of rebuilding the layout for each commit.
- Developer avatars flying on a plane above the files, gliding toward the centroid
  of each commit's affected files.
- One coordinated commit event: translucent connections from the avatar to every
  touched file, coloured to match the developer.
- Diff fragments descending into files for additions and rising out for removals.
  New files assemble; deleted files disappear after their connections are visible.
- File-type markers, readable labels and less detail at overview scale, with
  aggregation for large commits and crowded directories.
- Multiple repositories with shared contributor identities as a longer-term goal.

Explore a Tron-like neon style: steady glow, luminous connections and smooth motion.
Aim for a flashy presentation with no strobing and no loss of readability. Preserve
crisp file shapes and labels, and restrain brightness where activity overlaps.
Multiple visual styles are a later possibility; the probe does not implement themes.

## Prototype direction

The probe uses Bevy 0.20.1. The next step is a short, scripted visual study before
building Git ingestion or choosing the final architecture.

Compare avatar motion, beam shapes, diff direction and layout continuity using a
small synthetic history, including a deliberately crowded commit. The first study
should reveal whether the visual language remains readable in motion.

PNG export uses explicit frame indices, with one outstanding capture at a time and
confirmed writes before completion. The probe records logical state in `frames.csv`.
Encode the PNGs with external FFmpeg; streaming raw frames remains future work.

## Run the probe

```sh
cargo run --locked -- --help
cargo run --locked -- preview
cargo run --locked -- export frames
```

Preview loops four seconds; Escape closes the window. Export defaults to 120 frames
at 30 fps and 1280×720. The output directory must not already exist. Use `--frames`,
`--fps` and `--frame-delay-ms` to test export independently of rendering speed.
The probe has fixed file positions and a fixed camera; the visual goal above includes
features the probe does not implement yet.

Rendering changes also need the graphics export check and visual inspection:

```sh
cargo test --locked --test render_export -- --ignored
```

See [the video export issue](https://github.com/owenlamont/annealer/issues/9)
for the verified 10-second MP4 proof and playback results.

## Development

Rust 1.99.1, rustfmt and Clippy are pinned in `rust-toolchain.toml`.
Install [prek](https://prek.j178.dev/) to run the checks shared with CI.

```sh
prek install --install-hooks --overwrite
cargo run --locked -- --help
prek run --all-files
cargo test --locked --all-targets
```

CPU tests cover timing, lifecycle ordering and CLI behaviour without creating a
renderer. CI runs the checks on Linux, macOS and Windows. Rendering changes also need
a local GPU or software-renderer export and visual inspection.

The lint setup follows [ryl](https://github.com/owenlamont/ryl): rustfmt, strict
Clippy, Markdown and YAML checks, spelling, workflow linting and file hygiene.
Changes go through feature branches and pull requests.
