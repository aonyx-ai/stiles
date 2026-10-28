# API

<!--
goal: send a rule author to the right crate's own documentation.
non-goal: duplicating any of it here.
-->

The API documentation for each Stiles crate is on docs.rs.

## the table

| Crate                              | Holds                                                                                               |
| ---------------------------------- | --------------------------------------------------------------------------------------------------- |
| [whisker-plugin][whisker-plugin]   | `DeclaresRules` and `export_lints!`, which hand the lints of a crate to Stiles                      |
| [whisker-rust][whisker-rust]       | `RustLintPass`, one method per node kind                                                            |
| [whisker-types][whisker-types]     | `Diagnostic`, `Span`, `Suggestion`, `Severity`, `RuleId`, `DecoratedNode`, and the decoration types |
| [whisker-testing][whisker-testing] | `parse`, `decorate`, `execute`, `fixtures`, and `assert_diagnostic`                                 |

<!--
every link points at docs.rs and every crate sets publish = false, so nothing
resolves today. decide whether a page that cannot work yet ships or waits.
-->

[whisker-plugin]: https://docs.rs/whisker-plugin
[whisker-rust]: https://docs.rs/whisker-rust
[whisker-testing]: https://docs.rs/whisker-testing
[whisker-types]: https://docs.rs/whisker-types
