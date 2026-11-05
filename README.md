# Annealer

![Annealer logo](assets/annealer.svg)

A Git history visualiser exploring how software takes shape over time.

Annealer aims to retain Gource's moving developers and growing directory graph,
adding restrained 3D depth and animated diff fragments. The annealing metaphor is
a structure gradually settling as changes reshape the repository.

**Status: layout study.** Playback lays out a local repository or a built-in synthetic
history with persistent file positions, gliding contributors, coordinated commit
beams, labels and diff fragments. Preview and numbered PNG export share one
deterministic simulation. Built-in video encoding is not implemented. The repository
remains private; licensing and distribution are deferred, and Cargo publishing is
disabled.

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
The probe currently selects the neon theme; additional visual styles remain future work.

## Prototype direction

The probe uses Bevy 0.20.1. A short, scripted visual study should guide the final
playback architecture; local Git inspection runs independently of the renderer.

Compare avatar motion, beam shapes, diff direction and layout continuity using a
small synthetic history, including a deliberately crowded commit. The first study
should reveal whether the visual language remains readable in motion.

PNG export uses explicit frame indices, with one outstanding capture at a time and
confirmed writes before completion. The probe records logical state in `frames.csv`.
Encode the PNGs with external FFmpeg; streaming raw frames remains future work.

## Inspect local history

```sh
cargo run --locked -- inspect /path/to/repository --revision HEAD
```

Inspection reads complete local working or bare repositories without modifying them.
The experimental JSON contains parent-before-child commits, raw identities and paths,
and first-parent file changes.
See [the Git import issue](https://github.com/owenlamont/annealer/issues/5)
for implementation decisions and validation results.

Create a deterministic 100-commit test repository with
`uv run --script scripts/import_fixture.py NEW_REPOSITORY_DIRECTORY`.

## Play history

```sh
cargo run --locked -- preview
cargo run --locked --release -- preview --repository /path/to/repository
cargo run --locked --release -- export frames --repository /path/to/repository \
  --start 1000 --commits 200 --commit-seconds 0.1
```

Without `--repository`, the synthetic history plays, including a move between
directories, deletions and a 65-file commit. `--start` lays out and settles earlier
commits before playback; `--commits` limits what plays after it. Preview loops;
Escape closes the window. Export writes 1280×720 frames for the whole playback at
30 fps unless `--frames` or `--fps` say otherwise. The output directory must not
already exist. Commits touching more than 24 files aim beams at directories instead.

Repository playback caches the latest first-parent timeline and diff counts as JSON
under the platform user cache directory (`annealer/`), namespaced by a hash of the
canonical common Git directory, shared by linked worktrees. A resolved commit ID and
a format/import/timeline version
identify the snapshot; a different tip replaces the snapshot after a full import.
Playback range and speed reuse the complete timeline. Cache failures regenerate
without blocking playback; `--no-cache` skips cache reads and writes. Inspection
and repositories using Git replacement objects always import directly.
Messages, signatures, blob IDs and hunks are not cached. Bump `CACHE_VERSION` in
`src/timeline.rs` when cached semantics change.

Rendering changes also need the graphics export check and visual inspection:

```sh
cargo test --locked --test render_export -- --ignored
```

See [the video export issue](https://github.com/owenlamont/annealer/issues/9)
for the verified 10-second MP4 proof and playback results.

To reproduce the proof, install uv and FFmpeg/ffprobe with the libx264 encoder:

```sh
cargo build --locked -j 2
uv run --script scripts/video_proof.py target/debug/annealer video-proof
```

On Windows, use `target/debug/annealer.exe`. The output directory must be new.
Run the script with `--help` for output retention and cleanup instructions.

## Contributor aliases

Preview and export read `contributors.toml` from the user configuration directory:

- Linux: `~/.config/annealer/` (or `$XDG_CONFIG_HOME/annealer/` when set).
- macOS: `~/Library/Application Support/annealer/`.
- Windows: `%APPDATA%\annealer\`.

Override the file with `--contributors /path/to/contributors.toml`:

```toml
[[contributor]]
name = "Owen Lamont"   # display name
emails = ["owen@personal.example", "owen@work.example"]
```

Emails match case-insensitively; duplicate emails are errors. Matching names never merge
people with different nonempty emails. Unlisted emails keep separate identities,
displayed with the name from the last commit in playback order. Commits without an email
are grouped by author name. A missing default file is fine; an explicit missing file,
unknown key or malformed TOML is an error. Edits take effect on cache hits; original
authors and repository history remain unchanged.

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

## Acknowledgements

- [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme)
  (MIT) — file icons.
- [Bevy](https://bevy.org/) — rendering and application framework.
- [resvg](https://github.com/linebender/resvg) — startup SVG rasterisation.
- [gix](https://github.com/GitoxideLabs/gitoxide) — Git history access.
- [Gource](https://gource.io/) — inspiration for animated Git history.
