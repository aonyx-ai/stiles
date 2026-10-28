# Package and ABI versioning

<!--
goal: make a consumer see that their rules pin and their stiles version are one
thing that moves together, before a build breaks and tells them.
non-goal: how the layout check works. the plugin boundary page in the authoring
tree owns the mechanism, and this one never repeats it.
non-goal: whether a pinned repository is safe to run. pinning-and-trust owns
that, and a version match is not a trust decision.
-->

When you change your Stiles version, you change your rules pin too.

## What binds them

<!--
what binds them: why is my rules pin tied to my stiles at all. a rules
repository ships plugins, and a plugin only loads into the stiles it was built
for.
-->

Every `[[lints]]` git entry becomes a native library that Stiles loads, whether
Stiles downloads it or compiles it. Before Stiles calls into that library, it
checks that the library was built for this Stiles. When it was not, Stiles
refuses to load the library. No setting overrides that check.

## What is compared

<!--
what is compared: what actually has to match. a protocol version and two
fingerprints of stiles's own source. the compiler is not one of them, so any
rustc can have built either side.
-->

Three values that identify the Stiles source each side was built from.
[The plugin boundary](/authoring/explanation/plugin-boundary) explains them. Any
`rustc` can build either side.

## What it costs

<!--
what it costs: how often do I pay this. every stiles release before 1.0,
because major 0 promises nothing and a plugin loads only on the stiles it was
built for. reaching 1.0 turns that into a promise the major carries.
-->

Before 1.0, on every Stiles release. Version 0 has no compatibility guarantee,
so a rule loads only on the Stiles its publisher built it for. When you upgrade
Stiles, you also move your rules pin to a commit built for the new release.

From 1.0, only a major version can change the boundary. A minor release adds
only types and functions that old rules do not use.

## The order

<!--
the order: which do I move first. the stiles, then the rules pin to a commit
built against it. the reverse leaves you pinned to rules nothing can load.
-->

Stiles first, then the rules pin, to a commit built against that Stiles.

Stiles accepts either order. If you move the rules pin first, you pin rules
built for a Stiles you do not run yet. Your current Stiles cannot load them.
Renovate opens this pin update without a Stiles upgrade.

## How do I tell

<!--
how do I tell: what do I run. `stiles abi` prints the tag this stiles loads,
and a mismatch names both sides on stderr. troubleshooting carries the exact
messages.
-->

Run `stiles abi`. It prints the tag that a library has to match for this
Stiles to load it, for example `b7d58f06e26e0a22-aarch64-apple-darwin`. A
maintainer who publishes archives publishes one per tag, so you can look for an
archive with that tag.

When a library fails the handshake, Stiles prints the values from both sides on
stderr and stops. [Troubleshooting](/docs/how-to/troubleshooting) has the exact
messages.

## What a bot cannot do

<!--
what a bot cannot do: why did renovate not handle this. it watches the rules
repository's releases, so it cannot know your stiles moved, and it cannot pin a
commit nobody has pushed yet.
-->

The Renovate setup in [shared rules](/docs/how-to/shared-rules) follows the
releases of your rules repository. It does not detect a Stiles upgrade, and it
cannot pin a commit built for your new Stiles before someone pushes one. So you
upgrade Stiles by hand: move Stiles, then move the pin with it.
