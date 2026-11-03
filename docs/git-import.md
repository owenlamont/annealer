# Local Git import

The `inspect` command implements [local history import issue 5][issue]. Events contain
no Bevy types; playback/layout and contributor identity merging remain separate work.
The JSON inspection format is experimental and is not a versioned interchange contract.

```sh
cargo run --locked -- inspect /path/to/repository --revision HEAD
cargo run --locked -- inspect /path/to/repository --revision v1.0 > history.json
```

The command opens an existing working or bare repository, resolves one revision to a
commit, and imports every reachable parent. Annotated tags are peeled. Nothing is
fetched, checked out, staged, committed or written to the repository. Missing local
objects fail import; incomplete shallow/partial histories are not silently treated as
root commits. Repository paths and revision arguments are separate CLI values.

## Event contract

- `tip`: resolved full object ID; all parents retain their original order.
- `commits`: parent-before-child order, with full object ID as the tie-break among
  ready commits. Commit timestamp skew does not change graph ordering.
- `author` and `committer`: separate raw name/email byte arrays, the original signature
  header, signed epoch seconds and signed timezone offsets in seconds. Whitespace and
  non-UTF-8 bytes survive; no mailmap, alias merging or platform lookup is applied.
- `message`: original commit message bytes, without Unicode replacement.
- `changes`: root against an empty tree; every other commit against its first parent.
  Merge events retain every parent ID but show one coordinated first-parent change set.
  Empty commits remain events with an empty changes array.
- Paths and rename source paths are byte arrays, sorted lexicographically. Object IDs
  are full hexadecimal strings; entry modes are numeric Git mode bits, not permissions
  inferred from the current filesystem. Trees themselves are structural, not file events.
- Nonempty identical-content moves are `rename_candidate` events, with both paths and
  before/after entries. Copies are not detected. Edited moves remain addition/deletion
  pairs; exact matching is evidence, not proof of permanent file identity.
- Blob diffs use raw stored contents and histogram line matching. `text` includes
  insertion/removal counts and zero-based, half-open before/after hunk line ranges.
  Any NUL in the first 8,000 bytes of either blob classifies the pair as `binary`.
  Gitlinks retain mode/object IDs with `gitlink` classification; referenced repositories
  are not opened. Symlinks use their stored target bytes.

```mermaid
flowchart LR
  R[Resolve local revision] --> G[Collect reachable graph]
  G --> O[Order parents before children]
  O --> D[Compare first-parent trees]
  D --> B[Classify raw blob diffs]
  B --> J[Emit complete JSON]
```

Attributes, external diff drivers, textconv, worktree filters and user-selected diff
algorithms do not define blob classification or hunk output. The tree-diff cache uses
`Mode::ToGit` without worktree roots; rename matching is exact-content only, with no
fuzzy comparison budget to exhaust. This deliberately differs from attribute-aware
`git diff` for files marked `-diff` or with configured converters.

The importer retains the reachable graph and completed events in memory; the 64 MiB
object cache is not a total memory cap. Per-commit diff resources are cleared after
use. Large blobs and large histories need measurement before introducing streaming,
parallelism or persistent caches. Import failures produce no JSON; output-device errors
can still interrupt serialization. Keep diagnostics on stderr and data on stdout.

## Reproducible fixture and timing

Git is required for tests and fixture creation, not for production inspection. The
fixture creates 100 deterministic SHA-1 commits, with fixed identities/times and
monotonic text/file additions. Use a new directory; creation refuses an existing one.

```sh
bash scripts/import-fixture.sh "$HOME/agent-scratch/annealer-import-fixture"
cargo build --locked --release -j 2
/usr/bin/time -v target/release/annealer inspect \
  "$HOME/agent-scratch/annealer-import-fixture" --revision HEAD > history.json
```

Record the printed tip, Annealer revision, Rust/Git versions, OS, job count, object
cache budget and competing load. Use three warmups and fifteen measured processes;
report medians/ranges and RSS with JSON output redirected consistently. These are
warm-filesystem, cold-process imports. Repeat on packed and loose fixtures; do not
call the filesystem cold without an actual cache-control method.

Earlier release research compared gix 0.90.0 and git2 0.21.0 on fixed ryl/git2-rs tips,
with metadata and path/OID parity: warm medians 57.021 vs 130.039ms and
45.276 vs 134.623ms respectively. Traversal, renames and blob diffs were excluded.
Those results motivate gix, but do not predict this importer's full-event throughput.
The current command adds graph ordering, exact rename candidates, raw text diffs and
JSON serialization. Do not label the earlier numbers as an end-to-end import benchmark.

The native-Git tests exercise roots, empty commits, branch/merge history, explicit
revision selection, raw paths, file changes, binary/text content and repository
preservation. Extend semantic fixtures before expanding rename similarity or supporting
incomplete object graphs. Persistent derived-result caching is [issue 7][cache].

A debug smoke run on ryl at `2aa2313b7a0099ebcffae8c2dc26e78c59322ee5` imported
586 commits and 6,152 changes when exact rename candidates were expanded into their
source deletion/destination addition. All commit IDs, byte paths, modes and before/after
object IDs matched native Git under the first-parent policy. Fourteen exact rename
candidates were reported. This is a semantic check, not a release performance claim.

## Dependency sources

- [gix 0.90.0 revision parsing][revision] and [tree-change API][trees].
- [Rewrite options][rewrites]: `percentage = None` requests exact content matches.
- [Blob pipeline modes][modes]: `ToGit` keeps stored contents.
- [Raw blob diff][blob] provides histogram matching and line hunks.
- [Native Git raw diff format][raw] is the path/mode/object-ID test oracle.

[issue]: https://github.com/owenlamont/annealer/issues/5
[cache]: https://github.com/owenlamont/annealer/issues/7
[revision]: https://docs.rs/gix/0.90.0/gix/struct.Repository.html#method.rev_parse_single
[trees]: https://docs.rs/gix/0.90.0/gix/object/tree/diff/struct.Platform.html
[rewrites]: https://docs.rs/gix-diff/0.70.0/gix_diff/struct.Rewrites.html
[modes]: https://docs.rs/gix-diff/0.70.0/gix_diff/blob/pipeline/enum.Mode.html
[blob]: https://docs.rs/gix-diff/0.70.0/gix_diff/blob/index.html
[raw]: https://git-scm.com/docs/diff-format#_raw_output_format
