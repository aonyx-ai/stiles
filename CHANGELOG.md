<!-- markdownlint-disable-file MD013 MD024 -->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-10-30

### Added

- `ResolvedType::adt_kind`, `never_kind`, `result_kind`, and `option_kind`, and `AdtFlags::exhaustiveness`, return the recorded enums ([#558])

### Changed

- The plugin ABI moves to 0.7, because `ResolvedType` and `AdtFlags` now hold enums ([#558])

### Removed

- The `bool` setters of `ResolvedType` (`with_enum`, `with_never`, `with_result`, `with_option`) and `AdtFlags::new` ([#558])

## [0.2.0] - 2026-10-29

### Added

- `ResolvedType` and `AdtFlags` take enums beside their `bool` setters: `with_adt_kind`, `with_never_kind`, `with_result_kind`, `with_option_kind`, and `AdtFlags::from_exhaustiveness` ([#547])

### Changed

- `ProviderName` keeps its name private; build one with `ProviderName::new` ([#532])
- `generate_visitor` in stiles-codegen takes the language as a `LanguageName` ([#546])
- `has_span` in stiles-testing takes the byte range as `start..end` ([#546])

### Fixed

- `stiles-source` patches the stiles crates that lints take from crates.io, not only those from the stiles repository ([#548])

## [0.1.0] - 2026-10-28

### Added

- The first release of Stiles: write your own lints for your project's style conventions ([#518])

[0.1.0]: https://github.com/aonyx-ai/stiles/releases/tag/stiles@0.1.0
[#518]: https://github.com/aonyx-ai/stiles/pull/518
[0.2.0]: https://github.com/aonyx-ai/stiles/releases/tag/stiles@0.2.0
[#547]: https://github.com/aonyx-ai/stiles/pull/547
[#532]: https://github.com/aonyx-ai/stiles/pull/532
[#546]: https://github.com/aonyx-ai/stiles/pull/546
[#548]: https://github.com/aonyx-ai/stiles/pull/548
[0.3.0]: https://github.com/aonyx-ai/stiles/releases/tag/stiles@0.3.0
[#558]: https://github.com/aonyx-ai/stiles/pull/558
