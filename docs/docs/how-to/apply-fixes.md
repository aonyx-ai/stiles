# Apply fixes

<!--
goal: let a reader turn the findings that carry a fix into a reviewed commit.
non-goal: how the rounds work and when stiles writes nothing. the command line
reference owns that.
-->

Some findings carry a fix. The report marks each of them with a `fix:` note,
and a `help:` line counts them.

## Apply the fixes

Run the check with `--fix`:

```bash
stiles check --fix .
```

Stiles applies the fixes, writes the changed files, and reports the findings
that remain. When the run fails, Stiles writes no file. [Fixes][fixes] tells
how the fixes apply.

## Review the change

Read the diff before you commit it:

```bash
git diff
```

A rule attaches a fix only when the change is correct without review. A rule
can still have a bug, so treat the diff like a change that a person wrote.

## In CI

Run `stiles check` without `--fix` in CI. A finding with a fix then fails the
job like any other finding of its severity.

With `--fix`, the exit status counts only the findings that remain after the
fixes. The job can then pass while the repository still holds the findings.

[fixes]: /docs/reference/command-line#fixes
