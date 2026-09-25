# How Whisker works

<!--
goal: let a reader predict what a run costs, what it reads, and what a clean
result promises.
non-goal: the plugin boundary and the language provider's internals, which
architecture and the authoring pages own.
-->

## What a run does

<!--
the order: what happens when I run whisker check. resolve the configured
sources, get their rules from the cache, a prebuilt archive, or a compile, load
the project through the language's tooling, then discover, parse, check
coverage, and run the rules file by file, and report once at the end.
where the cost sits: which steps reach the network or a toolchain, and which
happen once per run rather than per file. rules are fetched and built once per
pin and reused after that.
-->

## The rules are yours

<!--
no built-in rules: what runs when I check. only the rules my configured sources
export, filtered by enable and disable. whisker ships none of its own.
native code: what a source costs. the first run compiles it, which takes
minutes and a rust toolchain, unless its publisher ships prebuilt archives.
built for this whisker: why an upgrade needs new rules. before 1.0 a rule
library loads only on the whisker it was built for, and the handshake refuses
the rest. package and abi versioning has the detail.
trust: why the configuration is the trust decision. loaded rules run inside
whisker's own process, prebuilt or compiled. pinning and trust has the rest.
-->

## Whisker asks your language's tooling

<!--
the tooling: how does whisker know about types. before it checks any file, it
loads the project through the language's own tooling, which for rust today is
rust-analyzer over the cargo workspace.
a project it understands: what can whisker check. only code that tooling places
in a project, so a stray file is outside it.
what loading does: why a check can run my code. it does what the tooling does
to understand the project, which for rust runs build scripts and lets cargo
fetch dependencies.
what it sees: what do rules know about. what the tooling sees and no more, so
rust counts cfg(test) code and gets no facts about proc-macro output.
the cost: why a small check on a big project is slow to start. loading the
project is paid once per run, before the first file.
-->

## What a clean run promises

<!--
clean means inspected: why is an uncovered file an error. running only the
rules that need no facts would return clean, and clean has to mean every rule
looked. troubleshooting has the reasons and their remedies.
missing evidence: what does a rule do without facts. it fails open, silent
where a fact is the reason to report, standing where a fact would have exempted
the code.
the preference: why prefer a miss to a guess. a finding whisker cannot justify
is wrong in a way nobody can see.
out of scope: what about files with no grammar. discovery never collects them,
so they are neither checked nor reported. file discovery has the rules.
-->
