# Stiles

Stiles is a linting platform built on [tree-sitter][ts]. It lints Rust and
TypeScript. Stiles ships no rules of its own: a project configures the ones it
wants, and those are exactly the ones it runs. Aonyx's own live in
[stiles-aonyx-rules][rules] and cover what Clippy does not, like derive
ordering, wildcard match arms, `matches!` macro usage, and the other style
rules defined in our `CLAUDE.md` files.

A rule implements the `RustLintPass` or `TypeScriptLintPass` trait, which
stiles generates from tree-sitter's grammar for that language. A rule that
needs type information reads decorations that stiles computes before any rule
runs, with [rust-analyzer][ra] for Rust and [tsrs][tsrs] for TypeScript.

## Status

Stiles is in early development. Check back soon.

## Quick start

Download the archive for your platform from the [releases page][releases],
check it against the `.sha256` beside it, and unpack it:

```bash
shasum -a 256 -c stiles-0.1.0-rc.5-aarch64-apple-darwin.tar.gz.sha256
tar -xzf stiles-0.1.0-rc.5-aarch64-apple-darwin.tar.gz
```

The archive unpacks to a directory that holds the binary, both licenses, and
this README. Move `stiles` to a directory on your `PATH`.

Stiles runs no rules until a project names some. Write
`.config/stiles.toml` at the top of your repository and pin a source of
rules to one commit:

```toml
[[lints]]
git = "https://github.com/aonyx-ai/stiles-aonyx-rules"
rev = "ffd02b34a39900a84045ff7bc0130885bf9f5732"
```

Then check the project:

```bash
stiles check .
```

The first run builds the rules, which takes as long as any Rust compilation.
Later runs reuse cargo's cache.

## Documentation

The [documentation][docs] covers the rest:

- [Install Stiles][install] — release archives, the GitHub Actions action,
  and building from source.
- [Command line][checking] — what ends a run, and what fails it.
- [Configuration][configuration] — every key of `.config/stiles.toml`.
- [Write a rule][custom-lints] — hook a node kind, test it, and give it
  options.
- [Prebuilt archives][prebuilt-lints] — publish compiled rules so a project
  skips the build.

[ARCHITECTURE.md](ARCHITECTURE.md) describes how Stiles works inside.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE)
  or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT)
  or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[checking]: https://aonyx-ai.github.io/stiles/docs/reference/command-line
[configuration]: https://aonyx-ai.github.io/stiles/docs/reference/configuration
[custom-lints]: https://aonyx-ai.github.io/stiles/authoring/how-to/write-a-rule
[docs]: https://aonyx-ai.github.io/stiles/
[install]: https://aonyx-ai.github.io/stiles/docs/how-to/install
[prebuilt-lints]: https://aonyx-ai.github.io/stiles/docs/reference/prebuilt-archives
[ra]: https://rust-analyzer.github.io/
[releases]: https://github.com/aonyx-ai/stiles/releases
[rules]: https://github.com/aonyx-ai/stiles-aonyx-rules
[ts]: https://tree-sitter.github.io/tree-sitter/
[tsrs]: https://github.com/oxc-project/tsrs
