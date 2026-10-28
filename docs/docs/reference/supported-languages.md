# Supported languages

<!--
goal: the one place that names each language Stiles lints and the tooling
behind it. every other page talks about languages in general and links here.
non-goal: how a language plugs in, which how-stiles-works and ARCHITECTURE.md
own.
-->

| Language   | Extensions                    | Project of a file                             | Type information from | Lint pass trait      |
| ---------- | ----------------------------- | --------------------------------------------- | --------------------- | -------------------- |
| Rust       | `.rs`                         | The Cargo workspace around the checked path   | [rust-analyzer][ra]   | `RustLintPass`       |
| TypeScript | `.ts`, `.mts`, `.cts`, `.tsx` | The project of the `tsconfig.json` nearest it | [tsrs][tsrs]          | `TypeScriptLintPass` |

Stiles parses every file with [tree-sitter][ts]. A language's own tooling then
supplies the facts a syntax tree cannot hold, such as types, by loading the
file's project. Stiles loads a language's projects only when it finds a file in
that language, so a repository needs a project file only for the languages it
holds.

One repository can hold several projects of a language. Each file belongs to
the project the table names, and Stiles loads each such project once.

[ra]: https://rust-analyzer.github.io/
[ts]: https://tree-sitter.github.io/
[tsrs]: https://github.com/oxc-project/tsrs
