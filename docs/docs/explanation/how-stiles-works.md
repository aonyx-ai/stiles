# How Stiles works

<!--
goal: let a reader predict what a run costs, what it reads, and what a clean
result promises.
non-goal: the plugin boundary and the language provider's internals, which
architecture and the authoring pages own.
-->

Stiles is a universal linting tool that allows you to write your own lints for
any (supported) language in similar style. It's based on a simple concept:
decorating tree-sitter trees with language-specific information.

## What a run does

1. Load rules (cached, prebuilt, or compile from source)
2. Load project in [language adapter](#language-tooling)
3. Parse [discovered files][file-discovery]
4. Run rules

And then it reports back to you!

## Your own rules

Stiles **does not ship with lint rules**. You have to
[configure][configuration-lints] or [build][write-a-rule] your own rules. This
comes with a few caveats:

**Lints may be slow to execute.** While we'll of course attempt to make Stiles
as performant as possible, we cannot make promises about the performance of
lints themselves. Additionally, running lints might require compiling them
first, which can be slow as well.

**Lints are native, trusted code.** Lints run as native code inside Stiles's
own process. Just like other tooling, make sure you trust the lints you include.
We may investigate methods to minimize this in the future.

## Language tooling {#language-tooling}

<!--
the tooling: how does stiles know about types. before it checks any file, it
loads the project through the language's own tooling.
a project it understands: what can stiles check. only code that tooling places
in a project, so a stray file is outside it.
what loading does: why a check can run my code. it does what the tooling does to
understand the project, including any build steps.
what it sees: what do rules know about. what the tooling sees and no more.
the cost: why a small check on a big project is slow to start. loading the
project is paid once per run, before the first file.
-->

Rules can read type facts as well as syntax. To supply them, Stiles loads your
project through the language's own tooling before it checks the first file.

The load has four consequences:

- **Stiles checks projects.** Stiles checks only the files that the tooling
  places in the loaded project.
- **The load can run code.** Stiles does what the tooling does to read a
  project, including any build steps the tooling runs.
- **Rules get only the facts the tooling computes.** Where the tooling computes
  no facts, rules get none.
- **The load happens once per run.** A check of one file in a large project
  still loads the whole project first, so the start of a run can be slow.

## What a clean run promises

<!--
clean means inspected: why is an uncovered file an error. running only the rules
that need no facts would return clean, and clean has to mean every rule looked.
troubleshooting has the reasons and their remedies.
missing evidence: what does a rule do without facts. it fails open, silent where
a fact is the reason to report, standing where a fact would have exempted the
code.
the preference: why prefer a miss to a guess. a finding stiles cannot justify
is wrong in a way nobody can see.
out of scope: what about files with no grammar. discovery never collects them,
so they are neither checked nor reported. file discovery has the rules.
-->

A clean run means every configured rule inspected every discovered file. Stiles
guarantees that by refusing any file it cannot fully analyze.

A file is **uncovered** when the language tooling cannot give Stiles the facts
its rules need about it. Stiles could still run the rules that only read
syntax. A clean result would then mean that only some rules inspected the file,
and you could not tell which. So the run exits 1.
[Troubleshooting](/docs/how-to/troubleshooting) lists the reasons and how to fix
each one.

Without the facts, a rule fails open. Where a fact is the reason to report, the
rule reports nothing. Where a fact would have exempted the code, the rule
reports it anyway. Stiles never guesses a fact, because nobody can tell a wrong
guess from a real finding.

Discovery collects only files in a language that Stiles has a grammar for, and
Stiles ignores every other file.
[File discovery](/docs/reference/file-discovery) has the rules.

[file-discovery]: /docs/reference/file-discovery
[configuration-lints]: /docs/reference/configuration#lints
[write-a-rule]: /authoring/how-to/write-a-rule
