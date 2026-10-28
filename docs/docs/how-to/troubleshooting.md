# Troubleshooting

<!--
goal: let a reader who has an error on stderr find the cause and the fix by
searching for the words they are looking at.
non-goal: explaining the designs behind the refusals. each entry links to the
explanation that owns its reasoning.
entries are keyed by the message, because that is what the reader has. the
messages live in the code, so this page goes stale in a way the explanation
pages do not. pin them with a test if this grows.
-->

When you run into an error in Stiles, check out this page to find out how to
resolve it.

## plugin (ABI x.y) incompatible with stiles (ABI x.y)

Stiles rules are built against a specific version of the Stiles ABI, and the
plugin was built against another one. A plugin with a lower ABI version needs a
rebuild against your Stiles. A plugin with a higher one needs a newer Stiles.

To resolve this, you have two options:

1. Move your rules pin to a commit built against your Stiles; or
2. Upgrade or downgrade your Stiles installation to the version the plugin was
   built for.

:::note

While Stiles is still in pre-1.0 development, every change to the ABI raises
its minor version, and Stiles loads only plugins built for its exact version.
This means ABI version `0.3` is incompatible with ABI version `0.2`.

:::

## the directory holds no dynamic library; delete it and run stiles again

Stiles has a cache for remote rules. Sometimes this cache breaks in unexpected
ways, for example when interrupting a Stiles run during build.

To resolve this problem, simply delete the listed directory and rerun Stiles.

## stiles cannot check FILE: there is no grammar for `.EXT` files

You've tried to make Stiles lint a file that it does not (yet) support. To do
this, you've most likely run `stiles check <filename.ext>`.

If you ran some other command, please file an issue on our Github.

## stiles analyzed no files under PATH

Stiles's file discovery found no files to lint. This usually means you've
ignored all files, or ran Stiles before writing code in a language it supports.

Double check the [ignore patterns][ignore] in `.config/stiles.toml`.

## a lint pass panicked while checking a NODE at byte N of FILE

A rule threw an error when checking a specific file.

Report this issue to the rule author, where possible together with the code that
caused the issue. You can try downgrading to a version of the rule that works,
or temporarily disable the rule altogether.

## two lints declare `RULE` for LANGUAGE: one in LIBRARY, one in LIBRARY

Two loaded lints report the same rule on the same language. Stiles turns rules
on and off, and hands them options, by their ID, so one ID must name one lint
per language. The cause is usually a rule copied from another and renamed in one
place only, or two rule repositories that chose the same ID.

If both lints are yours, rename the rule in one of them. If they come from two
rule repositories, remove one of them from your configuration and tell the
authors.

## the plugin exported a lint that declares no rules

A lint must name the rules it reports. Stiles refuses every diagnostic under a
rule the lint did not declare, so a lint that declares none could never report
anything.

Report this to the rule author. If the lint is yours, implement `DeclaresRules`
for it and return the IDs it reports.

## a lint pass reported `RULE` ..., but its plugin declares only RULES

A rule reported a diagnostic under an ID it did not declare. Stiles could not
turn that diagnostic off or hand it options, so it stops instead. The cause is
usually an ID that changed in one place but not the other.

Report this to the rule author. If the lint is yours, make its `DeclaresRules`
implementation return every ID it reports.

## cannot update the lock file ... because --locked was passed

You've configured a lint using a Git source without prebuilt archives. The
repository has no `Cargo.lock` committed, or the locked dependencies no longer
resolve.

File an issue with the owner of the rules. You can try to change the hash of the
lint to a version that did commit a `Cargo.lock`.

## failed to load the target project for analysis

<!--
what happened. stiles pointed rust-analyzer at the path you named and it found
no cargo project there. the cause line reads "discover a Cargo project at".
what do I do. run stiles from inside the project, or name the project directory
on the command line.
why does stiles need one. rust-analyzer loads the nearest workspace and runs
its build scripts before it can answer anything about a type.
-->

Stiles needs to load your Cargo project before it can check anything, and it
found no project at the path you gave it. The cause below the error reads
`discover a Cargo project at <path>`.

Run Stiles from inside your project, or pass the project directory:
`stiles check path/to/project`. Without a path, Stiles uses the current
directory.

## failed to load the TypeScript project for analysis

<!--
what happened. the tsconfig.json above a TypeScript file cannot be read. the
cause line names the configuration.
what do I do. install what the configuration extends, or exclude the file from
stiles.
why does stiles need one. the checker reads the configuration to know which
files make up the program and how to type them.
-->

Stiles checks each TypeScript file as part of the project of the
`tsconfig.json` nearest it, and it could not load that project. The cause
below the error reads `parse <tsconfig.json>` and names a package. Install
the project's dependencies, because the configuration extends them.

A TypeScript file with no `tsconfig.json` above it does not stop the check.
Stiles reports it as a file no provider covers, below.

To exclude files, add them to the `ignore` patterns in `.config/stiles.toml`.

## no decoration provider covers this file, so semantic rules cannot run

| Reason                                 | `help:` line                                                                                                     |
| -------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Outside the loaded root                | check the file from inside its own project, or exclude it from stiles                                            |
| Nothing loaded reaches it              | reference the file from a source the toolchain already loads, or exclude it from stiles                          |
| The toolchain excludes it              | remove the file from the toolchain's exclusion list, or exclude it from stiles                                   |
| Text differs from the toolchain's copy | re-run stiles                                                                                                    |
| No project file above it               | add the project file the toolchain reads, such as tsconfig.json, at or above the file, or exclude it from stiles |

[ignore]: /docs/reference/configuration#ignore
