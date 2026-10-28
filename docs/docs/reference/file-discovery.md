# File discovery

<!--
goal: answer "why was this file skipped" and "why was that one checked".
non-goal: the ignore pattern syntax, which configuration#ignore owns.
-->

Stiles has a fairly simple algorithm to discover which files to lint. However,
sometimes it may include or skip a file unexpectedly. Refer to this page to find
out how Stiles discovers files.

## Included files

Stiles walks through the files or directories you list in its `check`
arguments. Every file with an extension supported by Stiles is checked.
Anything else is always ignored.

| Extension                     | Language   | Project file    |
| ----------------------------- | ---------- | --------------- |
| `.rs`                         | Rust       | `Cargo.toml`    |
| `.ts`, `.mts`, `.cts`, `.tsx` | TypeScript | `tsconfig.json` |

Stiles loads a language's project only when it finds a file in that language,
so a repository with no TypeScript needs no `tsconfig.json`. A TypeScript file
belongs to the project of the `tsconfig.json` nearest it, so one repository can
hold several TypeScript projects.

If it turns out that an included file cannot be parsed by one of Stiles's
language adapters, Stiles will throw an error.

## Ignored files

There are several reasons Stiles might ignore a file:

| Skipped                      | Source                 |
| ---------------------------- | ---------------------- |
| Hidden files and directories | The walk               |
| `./.gitignore`               | The repository         |
| `./.ignore`                  | The repository         |
| `./.git/info/exclude`        | The repository         |
| global gitignore             | Your git configuration |
| `ignore` patterns            | `.config/stiles.toml`  |

## Checking specific files

You can override the ignore rules above by passing a specific file to
`stiles check`.
