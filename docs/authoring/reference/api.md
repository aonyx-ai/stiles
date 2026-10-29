# API

<!--
goal: send a rule author to the right crate's own documentation.
non-goal: duplicating any of it here.
-->

The API documentation for each Stiles crate is on docs.rs.

## Crates

| Crate                                  | Holds                                                                                               |
| -------------------------------------- | --------------------------------------------------------------------------------------------------- |
| [stiles-plugin][stiles-plugin]         | `DeclaresRules` and `export_lints!`, which hand the lints of a crate to Stiles                      |
| [stiles-types][stiles-types]           | `Diagnostic`, `Span`, `Suggestion`, `Severity`, `RuleId`, `DecoratedNode`, and the decoration types |
| [stiles-testing][stiles-testing]       | `parse`, `decorate`, `execute`, `fixtures`, and `assert_diagnostic`                                 |
| [stiles-rust][stiles-rust]             | `RustLintPass` and the Rust decorations                                                             |
| [stiles-typescript][stiles-typescript] | `TypeScriptLintPass` and the TypeScript decorations                                                 |

A lint depends on the crate of each language it checks. The
[supported languages][supported-languages] page lists them.

[stiles-plugin]: https://docs.rs/stiles-plugin
[stiles-rust]: https://docs.rs/stiles-rust
[stiles-testing]: https://docs.rs/stiles-testing
[stiles-types]: https://docs.rs/stiles-types
[stiles-typescript]: https://docs.rs/stiles-typescript
[supported-languages]: /docs/reference/supported-languages
