# Environment variables

<!--
goal: one table saying which variable changes what, and who reads it.
non-goal: explaining the cache or the token model. each has its own page.
-->

Whisker uses various environment variables listed below:

| Variable                 | Read by         | Effect                                                                                  |
| ------------------------ | --------------- | --------------------------------------------------------------------------------------- |
| `WHISKER_INSTALL_DIR`    | install script  | Where to put the binary. Defaults to `~/.local/bin`                                     |
| `WHISKER_CACHE_DIR`      | `whisker check` | Whisker's own [cache location][cache-layout]. Defaults to `${XDG_CACHE_HOME}/whisker`   |
| `XDG_CACHE_HOME`         | `whisker check` | See above.                                                                              |
| `GH_TOKEN`               | `whisker check` | When trying to access private repositories. Falls back to `$GITHUB_TOKEN`.              |
| `GITHUB_TOKEN`           | `whisker check` | See above.                                                                              |
| `CARGO`                  | `whisker check` | Location of the `cargo` binary to build lints. Defaults to the first `cargo` on `$PATH` |
| `WHISKER_GITHUB_API_URL` | `whisker check` | A GitHub Enterprise API to ask for prebuilt archives                                    |

[cache-layout]: /docs/reference/cache-layout
