# Package and ABI versioning

<!--
goal: make a consumer see that their rules pin and their whisker version are one
thing that moves together, before a build breaks and tells them.
non-goal: how the layout check works. the plugin boundary page in the authoring
tree owns the mechanism, and this one never repeats it.
non-goal: whether a pinned repository is safe to run. pinning-and-trust owns
that, and a version match is not a trust decision.
-->

When you change your Whisker version, you change your rules pin too.

## What binds them

<!--
what binds them: why is my rules pin tied to my whisker at all. a rules
repository ships plugins, and a plugin only loads into the whisker it was built
for.
-->

Every `[[lints]]` git entry becomes a native library that Whisker loads, whether
Whisker downloads it or compiles it. Before Whisker calls into that library, it
checks that the library was built for this Whisker. When it was not, Whisker
refuses to load the library. No setting overrides that check.

## What is compared

<!--
what is compared: what actually has to match. a protocol version and two
fingerprints of whisker's own source. the compiler is not one of them, so any
rustc can have built either side.
-->

Three values that identify the Whisker source each side was built from.
[The plugin boundary](/authoring/explanation/plugin-boundary) explains them. Any
`rustc` can build either side.

## What it costs

<!--
what it costs: how often do I pay this. every whisker release before 1.0,
because major 0 promises nothing and a plugin loads only on the whisker it was
built for. reaching 1.0 turns that into a promise the major carries.
-->

Before 1.0, on every Whisker release. Version 0 has no compatibility guarantee,
so a rule loads only on the Whisker its publisher built it for. When you upgrade
Whisker, you also move your rules pin to a commit built for the new release.

From 1.0, only a major version can change the boundary. A minor release adds
only types and functions that old rules do not use.

## The order

<!--
the order: which do I move first. the whisker, then the rules pin to a commit
built against it. the reverse leaves you pinned to rules nothing can load.
-->

Whisker first, then the rules pin, to a commit built against that Whisker.

Whisker accepts either order. If you move the rules pin first, you pin rules
built for a Whisker you do not run yet. Your current Whisker cannot load them.
Renovate opens this pin update without a Whisker upgrade.

## How do I tell

<!--
how do I tell: what do I run. `whisker abi` prints the tag this whisker loads,
and a mismatch names both sides on stderr. troubleshooting carries the exact
messages.
-->

Run `whisker abi`. It prints the tag that a library has to match for this
Whisker to load it, for example `b7d58f06e26e0a22-aarch64-apple-darwin`. A
maintainer who publishes archives publishes one per tag, so you can look for an
archive with that tag.

When a library fails the handshake, Whisker prints the values from both sides on
stderr and stops. [Troubleshooting](/docs/how-to/troubleshooting) has the exact
messages.

## What a bot cannot do

<!--
what a bot cannot do: why did renovate not handle this. it watches the rules
repository's releases, so it cannot know your whisker moved, and it cannot pin a
commit nobody has pushed yet.
-->

The Renovate setup in [shared rules](/docs/how-to/shared-rules) follows the
releases of your rules repository. It does not detect a Whisker upgrade, and it
cannot pin a commit built for your new Whisker before someone pushes one. So you
upgrade Whisker by hand: move Whisker, then move the pin with it.
