# Plugin boundary

<!--
goal: make a rule author understand why whisker refuses a plugin, and which part
of that they control.
non-goal: the mechanism. how a fingerprint is derived, and which change moves
which component, is whisker's own business and nobody here acts on it.
non-goal: the trust question, which pinning-and-trust owns, and the republish
schedule, which the prebuilt archives reference owns.
from 1.0, only when the major moves.
-->

Whisker loads a rule only when the rule and Whisker were built from the same
Whisker source.

## No stable ABI

<!--
no stable ABI: why does this check exist at all. rust gives two separately built
libraries no guarantee that they can call each other, so whisker checks a plugin
before it calls anything.
-->

Rust gives two separately built libraries no guarantee that they can call each
other safely. A rule is a dynamic library, and Whisker passes Whisker types into
it. Before Whisker calls into the rule, it checks that the rule was built
against the same Whisker source.

## What decides agreement

<!--
what decides agreement: what is actually compared. the whisker source each side
was built from. three values: a version and two fingerprints.
-->

The Whisker source that each side compiles against. Each side carries three
values: a protocol version and two fingerprints of that source. All three have
to match.

## The compiler is absent

<!--
the compiler is absent: must I match a toolchain. no. build a rule with whatever
rustc you like. this used to be required and is worth saying plainly, because
the old answer is still in people's heads.
-->

No. Whisker compares no compiler version, so you can build your rule with any
`rustc`.

## Guarding against silence

<!--
guarding against silence: why refuse instead of carrying on. rules fail open, so
a plugin built against drifted source would go quiet rather than wrong, and
nobody would see it.
-->

Rules fail open. A rule that cannot read the facts it needs reports nothing. A
plugin built against other Whisker source would report nothing too, and its
empty report looks the same as a clean one. Whisker refuses to load such a
plugin, so you see the mismatch as an error.

## No skip flag

<!--
no skip flag: can I override it. no. a mismatched layout is undefined behavior,
not a wrong answer.
-->

No. A call into a mismatched rule is undefined behavior: it can crash Whisker,
or corrupt its results without an error.

## What I control

<!--
what I control: what is mine to get right. one thing. pin the whisker crates to
the whisker that will load the plugin.
-->

One thing: pin `whisker-types`, `whisker-rust`, and `whisker-testing` to the
Whisker release that will load your rule.

## How often that costs me

<!--
how often that costs me: when do I rebuild. before 1.0, every whisker release.
-->

Before 1.0, every Whisker release. Version 0 has no compatibility guarantee, so
each release can change the boundary. After such a change, a rule loads only on
the Whisker you built it for. Plan to rebuild, and to republish any
[prebuilt archives](/authoring/reference/prebuilt-archives), with each release.

From 1.0, you rebuild only when the major version changes.

## What crosses safely

<!--
what crosses safely: what am I allowed to do inside a rule. use my own global
allocator, and panic without ending the process.
-->

- **Your own `#[global_allocator]`.** Each side frees the memory it allocates.
- **A panic.** Whisker reports a panic in your rule, and the process keeps
  running.

## What stays my problem

<!--
what stays my problem: what the handshake does not check. the rest of my
lockfile. it is outside the boundary, so commit it and keep it in step.
-->

The rest of your lockfile. The handshake checks only the Whisker crates, so the
versions of your other dependencies are yours to manage.

Commit your lockfile, and update it when you move your Whisker pin.
