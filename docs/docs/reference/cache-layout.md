# Cache layout

<!--
goal: let someone find, inspect, or delete what stiles cached.
non-goal: justifying the pin model. pinning-and-trust does that.
-->

Stiles keeps a cache of shared rules.

## Location and layout

By default, Stiles finds its caches according to these rules, taking the first
matching option:

1. `$STILES_CACHE_DIR`
2. `${XDG_CACHE_HOME}/stiles`, if `$XDG_CACHE_HOME` is set to an absolute path.
   Relative paths are ignored.
3. `~/.cache/stiles`

For most users, that means you'll find your cache in `~/.cache/stiles`.

Stiles keeps the following things in its cache:

| Path                             | Holds                                                             |
| -------------------------------- | ----------------------------------------------------------------- |
| `git/<remote>/<rev>/`            | A checkout of the pinned commit, including cargo target directory |
| `prebuilt/<remote>/<rev>/<tag>/` | Libraries unpacked from a published archive                       |

## Clearing caches

To force a fresh look for published archives, or refresh the downloaded
published archives, clear `prebuilt/<remote>/<rev>/`.

To force a fresh git fetch and compile of shared rules, clear
`git/<remote>/<rev>/`.
