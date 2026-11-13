# The plugin boundary for 1.0

Status: accepted. Tracks [stiles#640][issue-640].

From 1.0 an ABI version is one integer, and a plugin loads only on its own
version. A boundary that breaks whenever it grows would therefore cost one major
for each release that adds anything. This document designs a boundary that grows
by appending, so that an addition needs no new version, and a version moves only
when something that exists changes or goes away.

It also makes the semantic facts complete. The pass trait already covers every
node kind of the grammar. The facts a rule can read about those nodes do not:
they attach at positions someone chose by hand, for one rule.

## Goals

- An old plugin loads on a newer stiles of the same major, and works.
- A new plugin on an older stiles fails at load, with a message that names what
  it needs. It never loads and then finds nothing.
- A rule can read every semantic fact a decoration kind offers, at every node of
  the kinds that fact describes.
- A run computes only the facts that its loaded rules declare.
- A rule test stays a pure function of a tree and facts that the test supplies.
  It needs no analyzer and no Cargo project.

## Non-goals

- Inline suppression. A rule that misfires is fixed, scoped, or disabled in
  `[rules]`.
- A query interface. A pass reads facts and never calls an analyzer. A query
  waits until a rule proves that a decoration cannot carry what it needs.
- A stable ABI across majors. A major is allowed to break.

## What breaks today

| Change                              | What moves                        | Plugins it refuses |
| ----------------------------------- | --------------------------------- | ------------------ |
| A decoration is added               | `LANGUAGE_FINGERPRINT`            | All                |
| A decoration is changed             | Its key, `LANGUAGE_FINGERPRINT`   | All                |
| A field is added to `Diagnostic`    | `TYPES_FINGERPRINT`               | All                |
| A method is added to `LintPass`     | The pass vtable, `ABI_VERSION`    | All                |
| A field is added to the declaration | `ABI_VERSION`                     | All                |
| A `Language` or `Severity` is added | `TYPES_FINGERPRINT`               | All                |
| The grammar gains a node kind       | `VISITOR_FINGERPRINT`             | All                |
| A rule option gains a typed value   | `RuleOption`, `TYPES_FINGERPRINT` | All                |

Every row is an addition, except the second. Each refuses every plugin,
including the plugins that never touch what changed. The cause is twofold: one
hash covers the whole boundary, and plugins read most of the boundary as
laid-out structs whose shape is frozen into them.

Dispatch is not the problem. The generated `RustLintPass` lives inside the
plugin and matches on `node.kind()` by name. Only `LintPass::check_node`
crosses. A plugin built against an older grammar still sees every node of the
host's tree. It misses only the kinds its grammar did not know, and a kind that
was renamed.

## Principles

1. **Read by name, check at load.** A plugin names every fact, function, and
   node kind it uses. The host checks each name at load, and refuses with the
   names it lacks.
2. **Grow tables, not structs.** What a side offers the other is a table of
   functions that states its own length. A new entry goes at the end, so an
   older reader never reaches it.
3. **Freeze small values.** The values that do cross as laid-out data are small,
   and each one is frozen for the life of a major. A change makes a new value
   under a new name, beside the old one.
4. **No closed enum crosses by value.** An enum that a table call passes
   crosses as a fixed byte, and an unknown byte reads as unknown.

## The design

### Versions within a major

`ABI_VERSION` keeps the meaning that [#638][issue-638] gave it: a promise about
a release. Within a major, the boundary only grows. `check-abi` enforces that
against the latest release:

- `abi-surface.toml` records one entry per frozen value, per function of each
  table, per decoration key, and per node kind.
- An entry that changed or disappeared needs the next major.
- An entry that is new needs nothing.

The two fingerprints stay, but only for builds from unpublished source, as
[#639][pr-639] uses them. They no longer gate a published pair.

### The declaration and registration

The declaration shrinks to what the loader must read before it trusts anything:

```rust
#[repr(C)]
pub struct PluginDeclaration {
    pub abi_version: AbiVersion,
    pub types_fingerprint: u64,
    pub language_fingerprint: u64,
    pub provenance: Provenance,
    pub register: extern "C" fn(&Registrar) -> Registered,
}
```

`register` replaces `load`. The host passes a `Registrar`, a table of host
functions, and the plugin calls into it:

- `lint(language, factory, rules)` registers a pass.
- `reads(lint, key)` declares a decoration key the pass reads.
- `dispatches(lint, kind)` declares a node kind the pass handles.
- `option(rule, name, type)` declares an option a rule reads.
- `requires(table, length)` names the length of each host table the plugin was
  built against. That check is coarse: a plugin built against a longer table
  needs a newer stiles, even when it never calls the new functions.

A plugin never assembles a `Plugin` struct, so registration grows by new
functions, not new fields. `export_lints!` and the codegen write the calls. The
codegen cannot see which methods of the generated trait a pass overrides, so it
emits a `dispatches` call for every kind of the grammar.
[Decision 4](#decisions) says how that narrows later.

The host refuses a plugin when:

- the plugin requires a table longer than the host's,
- a lint declares no reads,
- a declared key has no provider in this stiles,
- a declared kind is not in the host's grammar, or
- a declared option conflicts with the configuration.

Each refusal names the item. A newer plugin on an older stiles therefore says
"needs the decoration `ExprType`", not "ABI mismatch".

### The pass

A pass stops being a stabby trait object. It is a table of optional entry points
with a length:

```rust
#[repr(C)]
pub struct PassTable {
    pub length: u32,
    pub configure: Option<extern "C" fn(Pass, &Options) -> Status>,
    pub check_node: Option<extern "C" fn(Pass, Node, &Sink) -> Status>,
    pub drop: extern "C" fn(Pass),
}
```

A hook added later goes at the end, such as `finish_file` or a project pass. The
host reads an entry only when the plugin's `length` covers it, and skips the
hook otherwise. Adding a hook is therefore not a break, and an optional
capability no longer has to live in the declaration.

### The node

Today a node crosses as a stabby struct of four fields, and a pass navigates it
with its own copy of the tree-sitter C library. In the new design a node crosses
as two words:

```rust
#[repr(C)]
pub struct Node<'a> {
    raw: TSNode,
    context: &'a Context,
}
```

`Context` is a host table: the source, the file, the decoration lookup, and the
navigation functions. `Node`'s methods in stiles-types call into it, so the
`TSNode` is opaque to the plugin, and only the host's tree-sitter reads it.
Rules keep the API they use today.

Today 14 call sites in stiles-aonyx-rules reach through `raw()` to tree-sitter.
Five read byte offsets, which `span()` already gives. Three read line numbers,
three take the host's grammar to parse a snippet such as a comment, and one
reads a previous sibling. `Context` therefore also offers the line of an offset,
the siblings of a node, and `parse(language, text)`, which returns a tree the
pass walks like the file's own.

[Decision 1](#decisions) removes `raw()`.

### Decorations

A decoration stays a frozen value that a provider computes before the walk. Two
things change: where it attaches, and who decides that it is computed.

**Attachment by node kind.** Each decoration kind attaches to every node of the
kinds it describes. The provider owns the mapping from those kinds onto
rust-analyzer or tsrs, once per decoration kind, so no rule waits for a position
to be added.

**Declared reads.** A provider computes only the keys that a loaded pass
declared. A run whose rules read no types infers no function body. That is the
laziness that matters, because rust-analyzer infers a whole body to type one
expression in it. [#607][issue-607] is this point.

A pass reads only what it declares, and a pass that declares nothing reads
nothing. A lookup of any other key would find nothing, and its rule would report
nothing. The host therefore fails the run at such a lookup, and names the rule
and the key. The host also refuses a plugin that registers a lint without a
`reads` call for it.

**Keys stay versioned.** A key already joins the name with a hash of the
definition. A changed decoration is a new key, and the provider may offer both
for the rest of the major. A plugin that reads the old key keeps working.

**A rule test supplies its own.** `stiles_testing::decorate` keeps working: a
test inserts the decorations it needs by hand. Declared reads do not apply
there, because no provider runs.

### The catalog for 1.0

The rule sweep of stiles-aonyx-rules and stiles-lint-rules set this list. The
count is roughly the number of rules that work around the missing fact today, or
read the decoration that it replaces.

| Decoration    | Attaches to                          | Carries                                                      | Rules |
| ------------- | ------------------------------------ | ------------------------------------------------------------ | ----- |
| `Target`      | `source_file`                        | crate name, target kind, test cfg, edition                   | 11    |
| `Resolution`  | every path and identifier that names | definition kind, crate, path, whether local, span when local | 6     |
| `Implements`  | every type item                      | traits it implements, with whether each is derived           | 9     |
| `Attributes`  | every item                           | attributes after `cfg` and `cfg_attr`, evaluated `doc` text  | 6     |
| `ExprType`    | every expression                     | the `ResolvedType` of today, plus variants and their shapes  | 3     |
| `Exposure`    | every item, associated items too     | public or internal from another crate                        | 1     |
| `Reexports`   | every path in a `use`                | as today                                                     | 1     |
| `FnSignature` | every function and closure           | as today, closures added                                     | 2     |

`ExprType` replaces `ResolvedType`, `AdtFlags`, and `EnumVariants` at their
fixed positions. It carries the variant shapes that a `matches!` fix needs,
which names alone cannot build. `Target` replaces the five `test_code.rs` copies
and every path guess.

TypeScript gets `ExprType` on every expression and `Resolution` on every
identifier from the tsrs checker. `ImportSource` stays. Its other decorations
fold into `ExprType`.

Five rules need another file, such as `outer_module_doc`, which would compare
the doc on a `mod` declaration with the header of that module's own file. They
wait for a project hook in [the pass table](#the-pass), not for a decoration.

[Decision 2](#decisions) covers macros.

### Output

The boundary pass reports through a `Sink`, a host table, instead of returning a
list of `Diagnostic` structs:

- `report(rule, severity, message, span)` starts a diagnostic and returns its
  handle.
- `origin(handle, span, message)` and `related(handle, span, message)` add a
  location.
- `edit(handle, span, replacement)` adds an edit to its fix, and `fix_message`
  names the fix.

A note, a help line, or a link to the rule's docs is a new function. The host
still enforces what it enforces today: a declared rule id, one file per fix, and
edits that do not overlap.

A rule does not see the sink. Its `check_` methods keep returning
`Vec<Diagnostic>`, and the adapter that `export_lints!` writes hands each
diagnostic to the sink. `Diagnostic` becomes a plugin-side type, so a field
added to it is not a boundary change at all.

### Options

Options read through the `Options` table: `names(rule, option)` as today, then
`integer`, `boolean`, and `string` as the rules need them. A rule declares each
option at registration, so a misspelled option in the configuration is an error,
not an absent value. stiles-lint-rules' `undocumented-option` then checks docs
against declarations, not source.

### Enums

`Language` and `Severity` stay enums in Rust, and each crosses a table call as
a fixed byte. The host reads a byte it does not know as unknown: it refuses a
lint of a language it does not know, and it refuses a severity it does not know.
Rules keep matching on the variants.

An enum inside a decoration stays an enum too. A rule matches its variants
exhaustively, which the conventions of stiles require. A new variant changes the
decoration's definition, so it makes a new key. A plugin that declares the old
key therefore loads only where a provider still offers it, and it never reads a
variant it does not know.

## What stays

- Stabby, for the frozen values: spans, rule ids, decoration values, and the
  panic a pass hands back.
- Panics caught at the boundary and returned as values.
- The provenance and the version rules of [#639][pr-639].
- `stiles-testing`'s shape: parse, decorate by hand, run, and assert.

## Migration

1. Land the tables behind the current API: `Node`, `Context`, `Sink`, and
   `PassTable`, with `export_lints!` writing the registration. Rules compile
   unchanged, except the call sites of `raw()`.
2. Add declared reads and load-time checks, and drop the decorations from
   `LANGUAGE_FINGERPRINT`.
3. Move `check-abi` to per-entry records.
4. Build the catalog, one decoration kind per pull request, `Target` first.
5. Migrate stiles-aonyx-rules and stiles-lint-rules once. Deleting the
   hand-written resolvers and the `test_code.rs` copies is the acceptance test.
6. Release, then add one decoration in the next release with no plugin rebuilt.
   That release is the proof, and 1.0 follows it.

Steps 1 to 3 break every plugin once, in one pre-1.0 version.

## Decisions

These were open questions, and each one took the recommendation.

1. **`raw()` goes.** It hands a pass the tree-sitter node, which the plugin
   navigates with its own copy of the C library. That ties every plugin to the
   host's tree-sitter, and a `TSNode` layout change would break them silently.
   Step 1 removes it, and `Context` offers what the 14 call sites use, as
   [the node](#the-node) lists.
2. **Macros come after the catalog.** Ten rules skip macro bodies. The leading
   idea is a decoration on each `macro_invocation` that carries the expansion
   as a tree, with spans mapped to the call site, so the walk visits it like
   any other code. Its design waits until step 4 is done, and the rest of the
   catalog does not wait for it.
3. **Diagnostics go through the sink.** A struct with a length prefix could
   also grow by appending, but only if stabby keeps declaration order, which
   is unverified. The sink needs no layout argument at all.
4. **Every pass declares every kind for 1.0.** The codegen cannot see which
   methods a pass overrides. A grammar that renames a kind therefore refuses
   every plugin. An attribute macro on the impl can narrow the list later.
5. **TypeScript ships as experimental.** It gets `ExprType` and `Resolution`,
   and it stays experimental until a rule reads them.

[issue-607]: https://github.com/aonyx-ai/stiles/issues/607
[issue-638]: https://github.com/aonyx-ai/stiles/issues/638
[issue-640]: https://github.com/aonyx-ai/stiles/issues/640
[pr-639]: https://github.com/aonyx-ai/stiles/pull/639
