# Write a rule

<!--
goal: take a reader from nothing to their own rule running in their project.
non-goal: explaining the boundary or the pipeline. both have their own pages.
-->

You start in an empty directory and finish with your own rule running in your
project. The guide follows the `doc_summary_break` example, which flags a doc
comment whose summary line has no blank line after it.

## Start from an example

<!--
where do I begin. doc_summary_break, or custom_lint for the same shape with more
comments.
-->

Copy
[`examples/doc_summary_break`](https://github.com/aonyx-ai/stiles/tree/main/examples/doc_summary_break)
from the Stiles repository into your project, for example to
`lints/doc_summary_break`. It is a complete rule crate with tests, and the rest
of this guide explains it.

[`examples/custom_lint`](https://github.com/aonyx-ai/stiles/tree/main/examples/custom_lint)
has the same files and structure, with more comments. Start there if you prefer
comments to code.

## The crate manifest

```toml title="lints/doc_summary_break/Cargo.toml"
[package]
name = "doc_summary_break"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
stiles-plugin = "=0.8.0"
stiles-rust = { version = "=0.8.0", default-features = false }
stiles-types = "=0.8.0"

[lib]
crate-type = ["cdylib"]
```

## Hook a node kind

A lint implements the lint pass trait of the language it checks. This example
checks Rust, so it implements `RustLintPass`. The
[supported languages][supported-languages] page names the trait of each
language.

```rust
impl RustLintPass for DocSummaryBreak {
    fn check_line_comment(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {
        if !is_doc_line(node) {
            return Vec::new();
        }

        let Some(next_doc_line) = get_next_doc_line(node) else {
            return Vec::new();
        };

        if !is_doc_line(&next_doc_line) || is_blank(&next_doc_line) {
            return Vec::new();
        }

        vec![
            Diagnostic::builder()
                .rule_id(RULE_ID)
                .severity(Severity::Warn)
                .message("the summary line needs a blank `///` line after it")
                .span(node.span())
                .build(),
        ]
    }
}
```

## Declare and export the rule

```rust
const RULE_ID: RuleId = RuleId::new("lint.doc-summary-break");

impl stiles_plugin::DeclaresRules for DocSummaryBreak {
    fn rules(&self) -> Vec<RuleId> {
        vec![RULE_ID]
    }

    fn reads(&self) -> stiles_plugin::Reads {
        stiles_plugin::Reads::none()
    }
}

stiles_plugin::export_lints![rust: [DocSummaryBreak]];
```

`reads` names the decorations the rule reads, and this rule reads none.

## Read a decoration

A decoration is a fact that the language's toolchain records on a node, such as
the type of an expression. The [API reference][api] lists the decorations of
each language. Name every decoration your rule reads in `DeclaresRules::reads`,
beside its rules:

```rust
use stiles_rust::decorations::{AdtFlags, ResolvedType};
use stiles_types::plugin::Reads;

impl stiles_plugin::DeclaresRules for WildcardMatchArm {
    fn rules(&self) -> Vec<RuleId> {
        vec![RULE_ID]
    }

    fn reads(&self) -> Reads {
        Reads::of::<ResolvedType>().and::<AdtFlags>()
    }
}
```

A rule that reads no decoration returns `Reads::none()`. Stiles computes only
the decorations that a loaded rule declares. A lookup of an undeclared one
therefore ends the run with the name of the rule and the decoration. Stiles also
refuses to load your rule when it offers no decoration with a declared name and
layout. That happens when your rule was built against another Stiles release.

Read a decoration from the node it describes:

```rust
let Some(resolved) = scrutinee.decoration::<ResolvedType>() else {
    return Vec::new();
};
```

A test attaches the decorations it needs with `stiles_testing::decorate`, so
declared reads do not apply there. A test therefore passes even when `reads`
leaves out a decoration that the rule reads, and only a real run catches it.

## Test the rule

```rust
use stiles_rust::RustLintPassAdapter;
use stiles_testing::{assert_diagnostic, assert_no_diagnostics, execute, parse};
use stiles_types::{Language, LintPass};

fn passes() -> Vec<Box<dyn LintPass>> {
    vec![Box::new(RustLintPassAdapter::new(DocSummaryBreak))]
}

#[test]
fn a_second_line_against_the_summary_is_flagged() {
    let tree = parse("/// Adds\n/// The rest.\nfn f() {}", Language::Rust);

    let diagnostics = execute(&tree, &mut passes());

    assert_eq!(diagnostics.len(), 1);
    assert_diagnostic(&diagnostics[0]).has_rule_id("lint.doc-summary-break");
}
```

## Offer a fix

A fix is a set of edits that apply together, all in the file of the diagnostic.
`stiles check --fix` applies every fix without review, so attach one only when
the change is correct in every case the rule reports.

```rust
let end = node.span().end();
let after_summary = Span::builder()
    .file(node.span().file())
    .start(end)
    .end(end)
    .build();
let fix = Fix::builder()
    .message("add a blank `///` line")
    .edits(vec![
        Edit::builder()
            .span(after_summary)
            .replacement("///\n")
            .build(),
    ])
    .build()
    .expect("one edit never overlaps itself");

vec![
    Diagnostic::builder()
        .rule_id(RULE_ID)
        .severity(Severity::Warn)
        .message("the summary line needs a blank `///` line after it")
        .span(node.span())
        .build()
        .with_fix(fix)
        .expect("the fix lies in the file of the diagnostic"),
]
```

`assert_fix` applies the fixes in rounds, as `--fix` does. It then checks the
result byte for byte, and checks that the rule reports nothing on it.

```rust
#[test]
fn the_fix_adds_a_blank_line_after_the_summary() {
    assert_fix()
        .tree(parse("/// Adds\n/// The rest.\nfn f() {}\n", Language::Rust))
        .passes(passes)
        .produces("/// Adds\n///\n/// The rest.\nfn f() {}\n");
}
```

## Run the rule

```toml title=".config/stiles.toml"
[[lints]]
path = "lints/doc_summary_break"
```

```bash
stiles check .
```

## Give the rule options

```toml title=".config/stiles.toml"
[rules.options."lint.repeated-primitive-params"]
boundary-attributes = ["shard", "procedure"]
```

```rust
impl RustLintPass for RepeatedPrimitiveParams {
    fn configure(&mut self, options: &RuleOptions) {
        self.boundary_attributes = options
            .names(RULE_ID, "boundary-attributes")
            .unwrap_or_default();
    }
}
```

## Next steps

<!--
where now. api, configuration, shared-rules, prebuilt archives.
-->

- [API reference](/authoring/reference/api) for every type and trait used here.
- [Configuration](/docs/reference/configuration#lints) for the other ways to
  name a rule source.
- [Shared rules](/docs/how-to/shared-rules) to use your rules in more than one
  project.
- [Prebuilt archives](/authoring/reference/prebuilt-archives) so the people who
  use your rules download them compiled.

[api]: /authoring/reference/api
[supported-languages]: /docs/reference/supported-languages
