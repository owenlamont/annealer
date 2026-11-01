# Annealer

A Git history visualiser exploring how software takes shape over time.

Annealer aims to retain Gource's moving developers and growing directory graph,
adding restrained 3D depth and animated diff fragments. The annealing metaphor is
a structure gradually settling as changes reshape the repository.

**Status: repository skeleton only.** The executable prints a placeholder message;
rendering, Git import and video export are not implemented. The repository is
private during early experiments, with a public release intended later. Licensing
and distribution will be decided before public release; Cargo publishing is disabled.

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

## Prototype direction

Start with a short, scripted visual study before building Git ingestion or choosing
the final architecture. Bevy is the proposed rendering engine, not yet a dependency.

Compare avatar motion, beam shapes, diff direction and layout continuity using a
small synthetic history, including a deliberately crowded commit. The first study
should reveal whether the visual language remains readable in motion.

Video export should use explicit frame times and controlled layout steps, sharing
the same scene logic as interactive playback. Start with numbered PNG frames encoded
by an external FFmpeg process; consider streaming raw frames later.

## Development

Rust 1.99.1, rustfmt and Clippy are pinned in `rust-toolchain.toml`.
Install [prek](https://prek.j178.dev/) to run the checks shared with CI.

```sh
prek install --install-hooks --overwrite
cargo run --locked
prek run --all-files
cargo test --locked --all-targets
```

The skeleton has no behavioural tests yet; `cargo test` currently checks compilation
of the test target. CI also builds and runs the placeholder on Linux, macOS and Windows.

The lint setup follows [ryl](https://github.com/owenlamont/ryl): rustfmt, strict
Clippy, Markdown and YAML checks, spelling, workflow linting and file hygiene.
Changes go through feature branches and pull requests.
