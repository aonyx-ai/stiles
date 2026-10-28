# Environment variables

<!--
goal: one table saying which variable changes what, and who reads it.
non-goal: explaining the cache or the token model. each has its own page.
-->

Stiles uses various environment variables listed below:

| Variable                | Read by        | Effect                                                                                  |
| ----------------------- | -------------- | --------------------------------------------------------------------------------------- |
| `STILES_INSTALL_DIR`    | install script | Where to put the binary. Defaults to `~/.local/bin`                                     |
| `STILES_CACHE_DIR`      | `stiles check` | Stiles's own [cache location][cache-layout]. Defaults to `${XDG_CACHE_HOME}/stiles`     |
| `XDG_CACHE_HOME`        | `stiles check` | See above.                                                                              |
| `GH_TOKEN`              | `stiles check` | When trying to access private repositories. Falls back to `$GITHUB_TOKEN`.              |
| `GITHUB_TOKEN`          | `stiles check` | See above.                                                                              |
| `CARGO`                 | `stiles check` | Location of the `cargo` binary to build lints. Defaults to the first `cargo` on `$PATH` |
| `STILES_GITHUB_API_URL` | `stiles check` | A GitHub Enterprise API to ask for prebuilt archives                                    |

[cache-layout]: /docs/reference/cache-layout
