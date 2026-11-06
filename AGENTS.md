# Annealer

Git history visualisation experiment, public and licensed MIT OR Apache-2.0.
The README separates the rendering probe from future goals. Keep Git import,
rendering and export work in separately scoped issues.
Reserve `docs/` for user-facing documentation rendered to HTML; keep research and
proof reports on issues.

## Workflow

- Use a feature branch and PR for changes; never merge without Owen's approval.
- Keep Cargo publishing disabled until distribution is settled.
- Use the installed engineering, Rust, docs and review skills as applicable.
- Review a PR before requesting merge approval, including the adversarial round.
- Stay near current stable Bevy releases; handle upgrades in feature branches.
- Read the [historical rendering reference](https://github.com/owenlamont/annealer/issues/12#issuecomment-6628967300)
  and upstream examples matching the current Bevy dependency before changing renderer code.

## Checks

Keep fixtures small and readable; build inline instead of committing recorded history.

```sh
prek install --install-hooks --overwrite
prek run --all-files
cargo test --locked --all-targets
cargo run --locked -- --help
```

Rust 1.99.1 is pinned with rustfmt and Clippy. Hook commands are shared with CI;
Cargo configuration and toolchain changes must trigger the Rust checks too.
Markdown prose wraps at 88 characters. CPU tests run without a GPU; changes to the
renderer also need a local export and visual inspection.

## Design constraints

Preserve file identity and spatial continuity across commits. Treat every commit
as one coordinated event connecting a developer to all affected files. Keep the
scene mostly planar with avatars above it. Drive export from explicit frame times,
not wall-clock rendering speed. Prototype visual readability before expanding scope.
Explore steady neon glow without strobing; visual effects must preserve readability.
