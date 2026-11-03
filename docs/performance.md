# Performance priorities and experiments

Assessment for [performance issue 8](https://github.com/owenlamont/annealer/issues/8),
against Annealer `ab25e9f`. No runtime optimisation is proposed without an Annealer
baseline. The synthetic renderer does not establish real-history or hardware-GPU costs.

## Priorities

Adapt [ryl's staged priorities][priorities] in order:

1. Stay within authorised scope; performance never authorises interface changes.
2. Preserve contracts: repository contents, raw identities and paths, file continuity,
   deterministic logical frame times, complete exports and supported platforms.
3. Keep behaviour predictable: explicit policies, bounded waits and visible failures.
4. Prefer measured runtime gains that justify complexity, low peak memory and simple,
   idiomatic code. Report speed, memory and code growth together.
5. Treat compile, lint and test feedback as early work alongside runtime performance;
   preserve verification coverage and stable CI status names.

YAML migration, scalar preservation and formatter idempotence are ryl contracts,
not Annealer requirements. Translate the method, not the domain-specific policy.

## Evidence worth transferring

Published measurements below are workload-specific reports, not rerun benchmarks.
Source links pin the implementations assessed; gains combine all changes in each PR.

- [Test consolidation PR 865][tests]: 221 targets became three; cold debug build
  173.4→84.6s, release workspace build 978.7→314.1s with dependencies retained;
  debug tests 68–71→24–27s. Batching subprocess fixtures contributed too.
  [Manifest][test-source]. Annealer has two integration targets and no repeated
  shared helper module; copying the registry now adds machinery without that cost.
- [Bounded ordered output PR 885][buffer]: at 20k files, check RSS 202→44 MiB;
  GitLab report RSS 966→42 MiB. Warm-cache loading remained 193 MiB.
  [Worker window][buffer-source]. Bound future import/diff/export queues and retain
  order; current export already waits for each capture and write. Item-count bounds
  alone do not bound oversized items.
- [One parse PR 874][parse]: traced changed-file check/diff path dropped 15 scanner
  constructions to one; three Markdown regions dropped ten to three.
  [Entry point][parse-source]. Share immutable commit/tree/diff facts after profiling
  repeated decoding; removing validation needs independent correctness evidence.
- [Scaling sweep PR 897][scaling]: Markdown fence formatting 0.5 MiB 589→98ms,
  4 MiB 33,068→739ms; slope 1.94→0.98. [Harness][scaling-source]. Vary commits,
  files per commit, total blobs and individual blob size separately; find nonlinear
  growth before tuning constants.

The test benchmark used four debug build jobs, sixteen release build jobs, eight test
threads, and disabled random property cases/persistence for timing. Default property
runs and seed replay were checked separately. The buffer benchmark used sixteen threads
against v0.27.0. These configurations are evidence controls, not recommended local limits.

Do not transplant YAML token tapes, quote caches, indentation renders, schema-specific
shortcuts or parser validation removal. Allocator swaps, SIMD, parallelism and release
LTO settings remain hypotheses until Annealer profiles justify their build/memory costs.

## Establish the build baseline first

[The logo-only CI run][ci] triggered full Rust checks without a Cargo cache:
Linux 9m05s, macOS 2m18s, Windows 13m22s. Runner startup was within two seconds;
macOS test compilation took 51.81s versus about 0.05s execution. One run demonstrates
where time went, not expected variance or a cache speedup.

The current workflow runs hooks in `Lint` and Clippy/tests again in each platform job.
Hooks include `cargo check` and Clippy. `cargo check`/Clippy artifacts do not replace
test code generation. The current manifest already disables dev debug information.

Prioritise these separately scoped experiments:

1. Cache Cargo outputs per OS, pinned toolchain and Cargo inputs. Compare cold miss
   and warm hit runs, restore/save time, bytes and wall time; retain all platforms.
2. Classify changed files inside an always-started workflow; skip Rust for prose/assets
   only. Test docs-only and Rust/config controls, stable statuses and failure propagation.
   Include Cargo, toolchain, hooks and workflow inputs; avoid workflow path filters
   that leave required checks pending.
3. Remove redundant Linux compilation after tracing gate/platform coverage. Require
   clean and incremental timings plus identical test inventory.
4. Measure import metadata, tree changes, renames and blob diffs separately. Pin tip,
   policy and cache budget; verify output parity, release time and RSS across sizes.
5. Profile rendering and PNG/encoding separately. Pin frames/rate/resolution, device
   and warmup; verify complete decoded output. Label hardware/software runs separately.

## Reproducible local controls

From an isolated checkout at the recorded revision, use a fresh target outside the
checkout. The first build is cold for Cargo outputs, not necessarily the OS page cache.
Record the toolchain, OS, CPU, job count, revision and competing load with the logs.

```sh
git rev-parse HEAD
rustc -Vv
cargo -V
mkdir -p "$HOME/agent-scratch/annealer-baseline"
export CARGO_TARGET_DIR="$HOME/agent-scratch/annealer-baseline/target"
export CARGO_BUILD_JOBS=2
/usr/bin/time -v cargo test --locked --all-targets
/usr/bin/time -v cargo test --locked --all-targets
/usr/bin/time -v cargo clippy --locked --all-targets --all-features -- -D warnings
cargo metadata --locked --no-deps --format-version 1
```

Use a new target path for a new cold sample; do not clean another session's target.
Repeat warm samples at least five times and report median/range, peak RSS and failures.
Linux `time -v` is not a whole-process-tree peak memory measurement. Keep cold compilation,
incremental compilation and test execution separate. Remove the dedicated target after
results are saved. CI baselines must use comparable runners and identical revisions;
local runs under competing builds do not establish a CI improvement.

No optimisation is implemented by this assessment. The [CI feedback follow-up][feedback]
should measure caching and change-aware checks first, with the controls above.

[priorities]: https://github.com/owenlamont/ryl/blob/2aa2313b7a0099ebcffae8c2dc26e78c59322ee5/docs/design/decisions.md#priorities
[tests]: https://github.com/owenlamont/ryl/pull/865
[test-source]: https://github.com/owenlamont/ryl/blob/bd91e8361fd9c71eaf78b80fbe22f770391381d0/Cargo.toml
[buffer]: https://github.com/owenlamont/ryl/pull/885
[buffer-source]: https://github.com/owenlamont/ryl/blob/baf116a29c0eedddc9689945420b262ad4770d58/src/cli_support.rs#L43
[parse]: https://github.com/owenlamont/ryl/pull/874
[parse-source]: https://github.com/owenlamont/ryl/blob/563f9f642e6339be7a20fbd5cd7eba1130d0f74c/src/format.rs#L221
[scaling]: https://github.com/owenlamont/ryl/pull/897
[scaling-source]: https://github.com/owenlamont/ryl/blob/2d00e9a4256c9306ad5ea6ded636e2b1c17114d2/scripts/scaling_check.py
[ci]: https://github.com/owenlamont/annealer/actions/runs/41642316940
[feedback]: https://github.com/owenlamont/annealer/issues/13
