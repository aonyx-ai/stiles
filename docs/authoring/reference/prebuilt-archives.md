# Publish prebuilt archives

<!--
goal: let a publisher of rules ship them compiled, so no consumer pays the
build.
non-goal: what a consumer sees when an archive is missing or broken. that is the
reference page of the same name in the usage tree.
-->

The maintainer of a rules repository can publish the rules compiled, so the
projects that use them download a library and skip the build.

## the archive

| Part     | Value                                               |
| -------- | --------------------------------------------------- |
| Name     | `<rev>-<tag>.tar.gz`                                |
| `<rev>`  | The commit a project pins                           |
| `<tag>`  | What `stiles abi` prints on the consumer's machine  |
| Digest   | A `.sha256` published beside the archive            |
| Contents | Regular files at the archive root, and nothing else |

## commit the lockfile

<!--
what else do I owe consumers. a committed lockfile, because a git entry builds
with --locked. without one every consumer's build fails, archives or not.
-->

Commit `Cargo.lock`. Stiles always builds a git entry with `--locked`, so
without a committed lockfile every consumer's build fails, whether or not you
publish archives.

## when the tag moves

<!--
when do my archives stop being found. when the boundary moves, because the tag
carries the fingerprints and the protocol floor. a stiles release that changes
either one wants a fresh archive.
why does a bad archive fail loudly. a library that fails the handshake carries a
tag that misdescribes it, so stiles ends the run rather than quietly compiling.
-->

| Event                                                 | New archive needed   |
| ----------------------------------------------------- | -------------------- |
| Any Stiles release before 1.0                         | Yes                  |
| A Stiles release after 1.0 that changes the boundary  | Yes                  |
| A new commit in your rules repository                 | Yes, for that commit |
| A consumer moves to a Stiles that prints the same tag | No                   |

The tag carries both fingerprints and the protocol version, so it changes
whenever the boundary does.

When a library from an archive fails the handshake, Stiles ends the run with an
error. The archive's tag matched this Stiles, so a failed handshake means the
archive is broken.
