# Harness

This package is the harness of the Stiles repository: the one place that
states which maintenance actions run here. It mounts the bundles that this
repository uses, and the command line that it builds turns each of them into a
command. A bundle carries a set of actions that projects adopt together, so the
harness names the bundle instead of each action in it.

## Usage

Run the harness from any directory of the repository:

```console
mise run rakko
```

Where mise supplies its environment, `rakko` is a shortcut for the same
command.

## Bundles

The harness mounts two bundles:

- [`rakko-baseline`][rakko-baseline] carries the actions that format and
  examine the files that any repository holds, whatever language it is
  written in, such as the formatters, the linters, and the check of the
  configuration of Renovate.
- [`rakko-rust-binary`][rakko-rust-binary] carries everything that a Rust
  project runs.

Stiles publishes binaries and no crate, so the harness mounts the binary
bundle and not `rakko-rust-library`. The crates still carry examples in their
documentation, so the harness mounts `test-rust-docs` on its own.

## Actions of this repository

The library of this package defines the checks that no bundle covers:

- `bench` measures stiles on a corpus of large projects, each pinned to one
  commit, and writes the results to `target/bench`. See [Benchmarks].
- `check-abi` compares the record of the plugin boundary with the one on
  `main`, and fails when a fingerprint moved and `ABI_VERSION` did not rise.
- `check-features` checks each language crate without its default features,
  the way a lint plugin depends on it.
- `check-self` builds stiles in release mode and checks this repository with
  the rules that `.config/stiles.toml` configures.
- `format-fixture-lint` and `test-fixture-lint` run `format-rust` and
  `test-rust` on the lint plugin that the tests of stiles-rust load. The root
  workspace excludes the plugin, because stiles-rust depends on it by path,
  so the Rust actions of the bundles do not reach it.
- `test-plugin-toolchain` builds the example plugin with the nightly toolchain
  and loads it into a stiles that the stable toolchain built.

The two plugins in `examples` declare a workspace of their own, so the Rust
actions of the bundles reach them without help.

## Benchmarks

`bench` builds stiles in release mode and checks each project of the corpus
with the rules that `.config/stiles.toml` pins, built against the working
tree:

| Project         | What it exercises                                     |
| --------------- | ----------------------------------------------------- |
| `stiles`        | This repository at 0.4.0, the small and fast baseline |
| `rust-analyzer` | A large Rust workspace, with build scripts            |
| `typescript`    | A large TypeScript project with one `tsconfig.json`   |
| `trpc`          | A TypeScript monorepo with one `tsconfig.json` each   |

```console
mise run rakko -- bench --project stiles --runs 10
```

Without `--project`, the run measures the whole corpus. A warm-up run first
builds the rules and downloads what each project needs. One run with
`stiles check --timings` then records the time of each phase, and hyperfine
times the runs that follow, with their peak memory.

`target/bench/<project>` holds the checkout, the export of hyperfine, and
the record of the phases. `target/bench/results.json` holds the measures of
every project in the [Bencher Metric Format]: the wall time, the peak memory,
and one measure per phase. The numbers vary by a third between runs on a
shared machine, so CI does not run the action on every pull request.

## Commands

### `pre-commit`

The command comes from [`rakko-pre-commit`][rakko-pre-commit]. It runs the
actions that guard a commit: the formatters first, in the order in which they
rewrite the tree, and then the checks that read what they wrote. `src/main.rs`
names both lists.

```console
mise run rakko -- pre-commit --fix
```

The hook that Git runs before a commit passes `--fix`, so that the formatters
rewrite the files that they can format. A run without the flag reports what a
commit has to repair, and it changes nothing.

The lists leave out the actions that resolve the dependencies of the project
again, such as `check-latest-deps`. They reach the network and take minutes,
which belongs in a job and not in front of every commit. A contributor runs
them by name, and CI runs them on every pull request.

[benchmarks]: #benchmarks
[bencher metric format]: https://bencher.dev/docs/reference/bencher-metric-format/
[rakko-baseline]: https://github.com/aonyx-ai/rakko/tree/main/bundles/rakko-baseline
[rakko-pre-commit]: https://github.com/aonyx-ai/rakko/tree/main/commands/rakko-pre-commit
[rakko-rust-binary]: https://github.com/aonyx-ai/rakko/tree/main/bundles/rakko-rust-binary
