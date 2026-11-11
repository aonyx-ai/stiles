# stiles reference --help

```console
$ stiles reference --help
Generate a reference page for a set of rules

Usage: stiles[EXE] reference [OPTIONS] --package <PACKAGE> --output <OUTPUT> [PATH]

Arguments:
  [PATH]  Path to the project that holds the package [default: .]

Options:
      --package <PACKAGE>  Package that re-exports every rule crate
      --output <OUTPUT>    File to write the page to
  -q, --quiet              Suppress informational messages
  -v, --verbose            Show additional detail
      --json               Output results as JSON
  -h, --help               Print help

```
