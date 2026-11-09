# Command line

<!--
goal: let a reader look up a flag, an exit code, or what a run does when
something fails.
non-goal: the design reasoning behind the strictness. how-stiles-works carries
that.
-->

The main way users interact with Stiles is through our CLI. We describe the
various options here.

## `stiles check`

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
| `--fix`           | Apply the fixes of the findings, then report what remains      |

### Fixes

A rule can attach a fix to a finding. A fix is a set of edits that apply
together, and a rule attaches one only when it is correct without review.
The report marks such a finding with a `fix:` note.

`--fix` applies the fixes in rounds. A round applies every fix that does not
overlap an earlier fix in the same file, and then checks the changed files
again. A fix that overlaps waits for the next round. The rounds stop when a
round changes nothing. Stiles then writes the changed files, and reports the
findings that remain. The exit status depends on those findings only.

Stiles writes nothing when the run fails. That covers every failure of
[early stops](#early-stops), and two failures of the fixes themselves:

- A file comes back to a text it held in an earlier round. The fixes of two
  rules undo each other, and the error names those rules.
- The files still change after 10 rounds. No flag raises this limit.

## `stiles rules`

To see which rules the configured lints declare, run this in the project. Each
line names a rule, the languages it runs on, and `off` when the project's
`[rules]` turn it off.

```bash
stiles rules [PATH]
```

## `stiles reference`

To generate a reference page for a set of rules, run this in the project that
holds them. The page is one HTML file that needs no other files.

```bash
stiles reference --package PACKAGE --output FILE [PATH]
```

`--package` names a package that re-exports each rule crate with
`pub use rule_crate;`. Each of those crates holds one public lint pass and one
public `RuleId` constant, built with `RuleId::new("...")`. The docs of the pass
become the entry of the rule. A `// DON'T` example and its `// DO` fix in the
`# Examples` section appear side by side, and doctests are left out.

## `stiles abi`

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
| `-v`, `--verbose` | Name each file that Stiles checked            |
| `--json`          | Write each result as one JSON object per line |

## Early stops

| Cause                                | Default      | With `--keep-going`     |
| ------------------------------------ | ------------ | ----------------------- |
| A directory Stiles cannot read       | Ends the run | Reported, run continues |
| An ignore file Stiles cannot parse   | Ends the run | Reported, run continues |
| A file Stiles cannot read or analyze | Ends the run | Reported, run continues |
| A rule that panics                   | Ends the run | Reported, run continues |
| A run that finds nothing to check    | Ends the run | Ends the run            |
| A fix that lies outside its file     | Ends the run | Ends the run            |
| Fixes that do not settle             | Ends the run | Ends the run            |

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

## Output streams

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
  "column": 5,
  "fix": {
    "message": "…",
    "edits": [{ "start": 120, "end": 124, "replacement": "…" }]
  }
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
| `fix`      | The fix of the diagnostic, or `null` without one     |

The edits of a fix come in the order of their spans. Each edit replaces the
bytes from `start` to `end` with `replacement`.

`stiles abi --json` writes the tag as one JSON string.
