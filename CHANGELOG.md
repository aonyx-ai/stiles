<!-- markdownlint-disable-file MD013 MD024 -->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-11-09

### Added

- The Rust provider says whether another crate can name an item ([#634])
- `stiles reference` generates a reference page for a set of rules ([#635])

### Changed

- The plugin ABI moves to 0.11, because the language fingerprint now covers the decoration that says whether another crate can name an item ([#636])

## [0.5.0] - 2026-11-09

### Added

- The tests of a rule check that its fixes produce the expected text ([#617])
- `stiles check --fix` applies the fixes of the findings and reports what remains ([#619])
- The Rust and TypeScript providers analyze the changed text of a file before it is written ([#618])
- The Rust provider types the values that decide a branch and names the variants of an enum ([#631])

### Changed

- A check of a Rust project resolves names on every core, so it runs faster on a machine with many cores ([#610])
- A check asks a toolchain for types only in the languages that a configured rule inspects ([#611])
- A check parses, decorates, and walks files on every core ([#612])
- A diagnostic holds one fix, and the edits of that fix apply together ([#616])
- A check against a `stiles-source` reuses the lockfile of its last build, so it no longer asks the registry on every run ([#626])

### Fixed

- A check with many findings reports them without reading every source file once per finding ([#622])

## [0.4.0] - 2026-11-05

### Changed

- The plugin ABI moves to 0.8, because the language fingerprint now covers decoration keys ([#576])

### Fixed

- Stiles refuses a plugin built against other decoration keys, instead of loading it and finding no decorations ([#576])
- A decoration's key no longer changes when only its documentation changes ([#576])
- `stiles-source` builds Git lint sources against the configured source even when prebuilt archives exist ([#577])
- `stiles-source` builds a Git lint source against the source when the lints require another version of the Stiles crates ([#590])

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
[0.4.0]: https://github.com/aonyx-ai/stiles/releases/tag/stiles@0.4.0
[#576]: https://github.com/aonyx-ai/stiles/pull/576
[#577]: https://github.com/aonyx-ai/stiles/pull/577
[#590]: https://github.com/aonyx-ai/stiles/pull/590
[0.5.0]: https://github.com/aonyx-ai/stiles/releases/tag/stiles@0.5.0
[#617]: https://github.com/aonyx-ai/stiles/pull/617
[#619]: https://github.com/aonyx-ai/stiles/pull/619
[#618]: https://github.com/aonyx-ai/stiles/pull/618
[#631]: https://github.com/aonyx-ai/stiles/pull/631
[#610]: https://github.com/aonyx-ai/stiles/pull/610
[#611]: https://github.com/aonyx-ai/stiles/pull/611
[#612]: https://github.com/aonyx-ai/stiles/pull/612
[#616]: https://github.com/aonyx-ai/stiles/pull/616
[#626]: https://github.com/aonyx-ai/stiles/pull/626
[#622]: https://github.com/aonyx-ai/stiles/pull/622
[0.6.0]: https://github.com/aonyx-ai/stiles/releases/tag/stiles@0.6.0
[#634]: https://github.com/aonyx-ai/stiles/pull/634
[#635]: https://github.com/aonyx-ai/stiles/pull/635
[#636]: https://github.com/aonyx-ai/stiles/pull/636
