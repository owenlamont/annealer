# Annealer

Private Git history visualisation experiment, intended for a later public release.
The README describes goals, not implemented features. The current binary is a
placeholder; keep Git import, rendering and export work in separately scoped issues.

## Workflow

- Use a feature branch and PR for changes; never merge without Owen's approval.
- Keep the repository private until Owen explicitly requests a visibility change.
- Keep publishing disabled until licensing and distribution are settled.
- Use the installed engineering, Rust, docs and review skills as applicable.
- Review a PR before requesting merge approval, including the adversarial round.

## Checks

```sh
prek install --install-hooks --overwrite
prek run --all-files
cargo test --locked --all-targets
cargo run --locked
```

Rust 1.99.1 is pinned with rustfmt and Clippy. Hook commands are shared with CI;
Cargo configuration and toolchain changes must trigger the Rust checks too.
Markdown prose wraps at 88 characters. The initial skeleton has no behavioural
tests; add tests when there is behaviour worth checking.

## Design constraints

Preserve file identity and spatial continuity across commits. Treat every commit
as one coordinated event connecting a developer to all affected files. Keep the
scene mostly planar with avatars above it. Drive export from explicit frame times,
not wall-clock rendering speed. Prototype visual readability before expanding scope.
