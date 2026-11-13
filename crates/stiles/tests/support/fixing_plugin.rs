//! Lint packages whose rules attach fixes
//!
//! Each package flags line comments by how they end, and attaches a fix
//! that changes the end. One package removes a final period, so its fixes
//! settle after one round. The other holds two rules that undo each other,
//! so its fixes never settle.

use std::path::Path;

/// How the rules of a written package treat the end of a line comment
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub(crate) enum Fixes {
    /// One rule removes a final period
    Settle,
    /// One rule adds a final `!`, and another removes it again
    Fight,
}

/// Writes a cdylib lint package named `name` whose rules attach `fixes`
pub(crate) fn write_fixing_lint_package(directory: &Path, name: &str, fixes: Fixes) {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let crates = std::fs::canonicalize(crates).expect("the crates directory should exist");
    let crates = crates.to_str().expect("the path should be UTF-8");

    std::fs::create_dir_all(directory.join("src")).expect("the package should be created");
    std::fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \
             \"2024\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n\
             stiles-plugin = {{ path = '{crates}/stiles-plugin' }}\nstiles-rust = {{ path = \
             '{crates}/stiles-rust', default-features = false }}\nstiles-types = {{ path = '{crates}/stiles-types' }}\n"
        ),
    )
    .expect("the manifest should be written");

    let period = Pass {
        name: "Period",
        rule: "custom.period",
        last: "'.'",
        insert: "None",
    };
    let add_bang = Pass {
        name: "AddBang",
        rule: "custom.add-bang",
        last: "'?'",
        insert: "Some(\"!\")",
    };
    let drop_bang = Pass {
        name: "DropBang",
        rule: "custom.drop-bang",
        last: "'!'",
        insert: "None",
    };
    let passes = match fixes {
        Fixes::Settle => period.source() + "\nstiles_plugin::export_lints![rust: [Period]];\n",
        Fixes::Fight => {
            add_bang.source()
                + &drop_bang.source()
                + "\nstiles_plugin::export_lints![rust: [AddBang, DropBang]];\n"
        }
    };
    let source = format!(
        "use stiles_rust::RustLintPass;\nuse stiles_types::{{DecoratedNode, Diagnostic, Edit, Fix, RuleId, Severity, Span}};\n{passes}"
    );
    std::fs::write(directory.join("src").join("lib.rs"), source)
        .expect("the source should be written");
}

/// A pass that changes the last character of a line comment
///
/// The pass flags a line comment whose last character is `last`. Its fix
/// removes that character, or replaces it with `insert`. A `last` of `'?'`
/// stands for any comment that does not end with `insert`, and its fix
/// appends `insert`. Each field is Rust source that the pass embeds.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
struct Pass {
    name: &'static str,
    rule: &'static str,
    last: &'static str,
    insert: &'static str,
}

impl Pass {
    /// Returns the source of the pass
    fn source(self) -> String {
        let Self {
            name,
            rule,
            last,
            insert,
        } = self;

        format!(
            r#"
/// Changes the end of a line comment
pub struct {name};

impl RustLintPass for {name} {{
    fn check_line_comment(&mut self, node: &DecoratedNode<'_>) -> Vec<Diagnostic> {{
        let text = node.text().trim_end();
        let span = node.span();
        let end = span.start() + text.len();
        let insert: Option<&str> = {insert};
        let (start, replacement) = match ({last}, insert) {{
            ('?', Some(insert)) if !text.ends_with(insert) => (end, insert),
            ('?', _) => return Vec::new(),
            (last, _) if !text.ends_with(last) => return Vec::new(),
            (_, insert) => (end - 1, insert.unwrap_or("")),
        }};
        let edit = Edit::builder()
            .span(Span::builder().file(span.file_path().clone()).start(start).end(end).build())
            .replacement(replacement)
            .build();
        let fix = Fix::builder()
            .message("change the end of the comment")
            .edits(vec![edit])
            .build()
            .expect("the fix should build");
        let diagnostic = Diagnostic::builder()
            .rule_id(RuleId::new("{rule}"))
            .severity(Severity::Error)
            .message("the comment ends wrongly")
            .span(span)
            .build()
            .with_fix(fix)
            .expect("the fix should attach");
        vec![diagnostic]
    }}
}}

impl stiles_plugin::DeclaresRules for {name} {{
    fn rules(&self) -> Vec<RuleId> {{
        vec![RuleId::new("{rule}")]
    }}

    fn reads(&self) -> stiles_plugin::Reads {{
        stiles_plugin::Reads::none()
    }}
}}
"#
        )
    }
}
