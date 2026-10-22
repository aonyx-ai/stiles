# Pinning and trust

<!--
goal: separate three things a reader conflates, a pin, a digest and the
handshake, and land on where trust actually lives.
non-goal: how to pin. shared-rules covers the mechanics.
-->

A pinned commit, a published digest and a successful load each look like a
reason to trust a rules repository. None of them is.

## Why a commit

<!--
why a commit: why not a branch. a linter that changes underneath a project is
not one anyone can act on.
-->

A branch moves when someone pushes to it, so the rules on a branch can change
while your project stays the same. Whisker requires a `rev` for every `git`
entry.

## Why the full hash

<!--
why the full hash: why not an abbreviation. it grows ambiguous as a repository
gains objects.
-->

An abbreviated hash is unique when you copy it, and can become ambiguous as the
repository gains objects. Whisker requires the full 40-character hash.

## A pin is a cache key

<!--
a pin is a cache key: what else does the commit buy. an immutable tree, so no
refresh and no second question to the remote.
-->

A commit names a tree that never changes. Whisker stores a checkout under the
commit hash and never refreshes it, so a second run sends no request to the
remote. [Cache layout](/docs/reference/cache-layout) shows where Whisker stores
it.

## What the digest proves

<!--
what the digest proves: does a matching digest mean safe. no. the same publisher
writes the archive and the digest.
-->

That the download arrived whole. The `.sha256` beside a prebuilt archive comes
from the same publisher as the archive. Whisker compares the download against it
before unpacking anything, so a truncated or corrupted download never reaches
the disk as files. The digest does not tell you who built the archive, or what
is in it.

## What the handshake proves

<!--
what the handshake proves: does loading successfully mean safe. no. only that
both sides were built against the same whisker source, and the compiler is not
part of it.
-->

That the library was built against the same Whisker source as the Whisker that
loads it. The handshake checks nothing about the source either side was compiled
from, the compiler, or the dependencies.

## Neither is trust

<!--
neither is trust: what have we actually established. two compatibility checks
and no trust decision, said outright.
-->

The pin and the digest check what you downloaded, and the handshake checks how
it was built. None of them checks what the code does.

## Where trust comes from

<!--
where trust comes from: what decides it then. the configuration. a named
repository runs unsandboxed in-process, and opening its library runs some of its
code before the handshake reads anything.
-->

Your configuration. Whisker fetches the repository that a `[[lints]]` entry
names, compiles it, and loads it into its own process with no sandbox. Whisker
has to open the library before it can run the handshake, and opening a library
already runs some of its code. The handshake result determines whether Whisker
calls into the rules. Their code runs either way.

## The instruction

<!--
the instruction: what do I do with that. pin what you would run as a build
script, and read the diff when you move a pin.
-->

Pin only what you would run as a build script, and read the diff when you move a
pin.
