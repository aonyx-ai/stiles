# Command line

<!--
goal: let a reader look up a flag, an exit code, or what a run does when
something fails.
non-goal: the design reasoning behind the strictness. how-stiles-works carries
that.
-->

The main way users interact with Stiles is through our CLI. We describe the
various options here.

## check

Normally you'll invoke Stiles through `stiles check .`. There's some more
options you can pass though:

```bash
stiles check [OPTIONS] [PATH]
```

| Argument          | Meaning                                                        |
| ----------------- | -------------------------------------------------------------- |
| `PATH`            | The directory or file to check. Defaults to `.`                |
| `--keep-going`    | Report each failure and continue. The run still exits non-zero |
| `--deny-warnings` | Fail on warnings as well as errors                             |

## rules

To see which rules the configured lints declare, run this in the project. Each
line names a rule, the languages it runs on, and `off` when the project's
`[rules]` turn it off.

```bash
stiles rules [PATH]
```

## abi

To find the ABI version of a Stiles binary, you can run this command. It takes
no options.

```bash
stiles abi
```

## Global options

With every Stiles command, you can use the following parameters to modify its
output or behavior:

| Option            | Meaning                                       |
| ----------------- | --------------------------------------------- |
| `-h`, `--help`    | Print help                                    |
| `-V`, `--version` | Print the version                             |
| `-q`, `--quiet`   | Hide the `help:` lines                        |
| `-v`, `--verbose` | Name each file as Stiles checks it            |
| `--json`          | Write each result as one JSON object per line |

## What ends a run early

| Cause                                | Default      | With `--keep-going`     |
| ------------------------------------ | ------------ | ----------------------- |
| A directory Stiles cannot read       | Ends the run | Reported, run continues |
| An ignore file Stiles cannot parse   | Ends the run | Reported, run continues |
| A file Stiles cannot read or analyze | Ends the run | Reported, run continues |
| A rule that panics                   | Ends the run | Reported, run continues |
| A run that finds nothing to check    | Ends the run | Ends the run            |

## Exit status

Stiles exits with one of two statuses:

| Status | Meaning                                                                         |
| ------ | ------------------------------------------------------------------------------- |
| `0`    | Nothing failed the run                                                          |
| `1`    | A diagnostic met the failure threshold, or a file could not be read or analyzed |

<!--
which severities count. the table with and without --deny-warnings.
will the output match the exit code. yes, a promoted warning renders as an
error.
-->

| Severity | Fails by default | Fails with `--deny-warnings` |
| -------- | ---------------- | ---------------------------- |
| `Error`  | Yes              | Yes                          |
| `Warn`   | No               | Yes                          |
| `Info`   | No               | No                           |
| `Help`   | No               | No                           |

## Where output goes

<!--
stdout or stderr. the result of a command goes to stdout: the diagnostics of
check, the tag of abi. errors and notes go to stderr. help lines and the
--verbose file names go to stdout, or to stderr with --json, so that stdout
then holds only JSON.
-->

| Command        | stdout                                     | stderr           |
| -------------- | ------------------------------------------ | ---------------- |
| `stiles check` | Diagnostics, `help:` lines, and file names | Errors and notes |
| `stiles abi`   | The tag                                    | Errors only      |

With `--json`, stdout holds only the JSON. The `help:` lines and the file
names of `--verbose` move to stderr. A clean `stiles check` prints nothing
to stdout in either mode.

Because `stiles abi` writes its tag to stdout, you can use it in a shell
substitution, for example `tag=$(stiles abi)`.

## JSON output

`stiles check --json` writes one object per diagnostic:

```json
{
  "rule": "lint.todo-comment",
  "severity": "warning",
  "message": "…",
  "file": "src/lib.rs",
  "start": 120,
  "end": 124,
  "line": 7,
  "column": 5
}
```

| Field      | Meaning                                              |
| ---------- | ---------------------------------------------------- |
| `rule`     | The rule id                                          |
| `severity` | `error`, `warning`, `note`, or `help`                |
| `message`  | The text of the diagnostic                           |
| `file`     | The file, as Stiles found it                         |
| `start`    | The byte offset where the diagnostic starts          |
| `end`      | The byte offset where the diagnostic ends            |
| `line`     | The line of `start`, counting from 1                 |
| `column`   | The column of `start` in characters, counting from 1 |

`stiles abi --json` writes the tag as one JSON string.
