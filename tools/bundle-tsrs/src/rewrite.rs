//! Rewrites the paths of a tsrs file so that it compiles as a module
//!
//! Inside the bundle, each tsrs crate is a module at the root, so a path
//! that names a crate must name the module instead:
//!
//! - `tsrs_core::x` becomes `crate::tsrs_core::x`.
//! - In a file of `tsrs_core`, `crate::x` becomes `crate::tsrs_core::x`, and
//!   `$crate::x` becomes `$crate::tsrs_core::x`.
//! - A macro that `#[macro_export]` exports lives at the root of the crate
//!   that defines it. It moves to the root of the bundle, so a path to it
//!   becomes `crate::name!`.
//! - `env!("CARGO_MANIFEST_DIR")` names the directory of the bundle, so it
//!   gains the directory of the crate inside it.
//!
//! The rewrite works on tokens, so it also reaches paths inside macro bodies,
//! and it never touches a comment or a string. Each edit splices the byte
//! range of a token, so every other byte of the file stays as it was.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use proc_macro2::Delimiter;
use proc_macro2::Spacing;
use proc_macro2::TokenStream;
use proc_macro2::TokenTree;

/// The name of a crate: a tsrs crate, which is also its module, or the bundle
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub(crate) struct CrateName(String);

impl CrateName {
    pub(crate) fn new(name: &str) -> Self {
        Self(name.to_owned())
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A file after the rewrite
#[derive(Clone, Eq, PartialEq, Debug)]
pub(crate) struct Rewritten {
    /// The text of the file with its paths rewritten
    text: String,
    /// The paths that `include_str!` and `include_bytes!` read, as the file names them
    includes: Vec<String>,
}

impl Rewritten {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn includes(&self) -> &[String] {
        &self.includes
    }
}

/// Rewrites the files of the crates that one bundle holds
#[derive(Clone, Debug)]
pub(crate) struct Rewriter {
    crates: BTreeSet<CrateName>,
    /// Every exported macro, with the number of times a crate exports it
    exported: BTreeMap<String, usize>,
}

/// One replacement of a byte range of the file
#[derive(Clone, Eq, PartialEq, Debug)]
struct Edit {
    range: Range<usize>,
    replacement: String,
}

impl Rewriter {
    pub(crate) fn new(crates: BTreeSet<CrateName>) -> Self {
        Self {
            crates,
            exported: BTreeMap::new(),
        }
    }

    /// Records the macros that `text` exports with `#[macro_export]`
    ///
    /// Every file of the bundle must pass through here before the first
    /// rewrite, because a path in one crate can name a macro of another.
    pub(crate) fn collect_exported_macros(&mut self, text: &str) -> Result<()> {
        let stream = lex(text).context("failed to read the tokens of the file")?;
        collect_macros(stream, &mut self.exported);
        proc_macro2::extra::invalidate_current_thread_spans();
        Ok(())
    }

    /// Checks that the exported macros can share the root of the bundle
    pub(crate) fn check_exported_macros(&self) -> Result<()> {
        for (name, count) in &self.exported {
            ensure!(
                *count == 1,
                "{count} tsrs crates export a macro named {name}, and the bundle has one root"
            );
            ensure!(
                !self.crates.contains(&CrateName::new(name)),
                "the macro {name} has the name of a tsrs crate"
            );
        }
        Ok(())
    }

    /// Rewrites `text`, a file of the crate `module`
    pub(crate) fn rewrite(&self, text: &str, module: &CrateName) -> Result<Rewritten> {
        let stream = lex(text).context("failed to read the tokens of the file")?;
        let mut edits = Vec::new();
        let mut includes = Vec::new();
        self.visit(stream, module, &mut edits, &mut includes)
            .context("failed to find the paths to rewrite")?;
        proc_macro2::extra::invalidate_current_thread_spans();

        Ok(Rewritten {
            text: apply(text, edits).context("failed to apply the rewrite")?,
            includes,
        })
    }

    fn visit(
        &self,
        stream: TokenStream,
        module: &CrateName,
        edits: &mut Vec<Edit>,
        includes: &mut Vec<String>,
    ) -> Result<()> {
        let tokens: Vec<TokenTree> = stream.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            let ident = match token {
                TokenTree::Group(group) => {
                    self.visit(group.stream(), module, edits, includes)
                        .context("failed to rewrite a group of tokens")?;
                    continue;
                }
                TokenTree::Ident(ident) => ident,
                TokenTree::Punct(_) | TokenTree::Literal(_) => continue,
            };
            let name = ident.to_string();
            let range = ident.span().byte_range();

            if name == "crate" {
                if is_path_separator(&tokens, index + 1) && !self.names_macro(&tokens, index + 3) {
                    edits.push(Edit {
                        range: range.end..range.end,
                        replacement: format!("::{module}"),
                    });
                }
            } else if self.crates.contains(&CrateName::new(&name)) {
                if follows_path_or_field(&tokens, index) {
                    continue;
                }
                if is_path_separator(&tokens, index + 1) && self.names_macro(&tokens, index + 3) {
                    let start = tokens[index + 3].span().byte_range().start;
                    edits.push(Edit {
                        range: range.start..start,
                        replacement: "crate::".to_owned(),
                    });
                } else {
                    edits.push(Edit {
                        range: range.start..range.start,
                        replacement: "crate::".to_owned(),
                    });
                }
            } else if name == "env" {
                let Some((literal, end)) =
                    macro_literal(&tokens, index).context("failed to read env!")?
                else {
                    continue;
                };
                if literal == "CARGO_MANIFEST_DIR" {
                    edits.push(Edit {
                        range: range.start..end,
                        replacement: format!(
                            "concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{module}\")"
                        ),
                    });
                }
            } else if name == "include_str" || name == "include_bytes" {
                let Some((literal, _)) =
                    macro_literal(&tokens, index).context("failed to read an include")?
                else {
                    continue;
                };
                includes.push(literal);
            }
        }
        Ok(())
    }

    /// Returns whether `tokens[index]` ends a path with the name of an exported macro
    ///
    /// The name ends the path both in a call, `crate::free!()`, and in an
    /// import, `pub use crate::free;`. A segment that a path continues past
    /// names a module, whatever its name.
    fn names_macro(&self, tokens: &[TokenTree], index: usize) -> bool {
        let Some(TokenTree::Ident(ident)) = tokens.get(index) else {
            return false;
        };
        self.exported.contains_key(&ident.to_string()) && !is_path_separator(tokens, index + 1)
    }
}

fn lex(text: &str) -> Result<TokenStream> {
    text.parse::<TokenStream>()
        .map_err(|error| anyhow::anyhow!("{error}"))
        .context("failed to lex the file")
}

/// Records each `macro_rules!` whose attributes include `#[macro_export]`
fn collect_macros(stream: TokenStream, exported: &mut BTreeMap<String, usize>) {
    let tokens: Vec<TokenTree> = stream.into_iter().collect();
    for (index, token) in tokens.iter().enumerate() {
        let ident = match token {
            TokenTree::Group(group) => {
                collect_macros(group.stream(), exported);
                continue;
            }
            TokenTree::Ident(ident) => ident,
            TokenTree::Punct(_) | TokenTree::Literal(_) => continue,
        };
        if ident != "macro_rules" || !is_punct(&tokens, index + 1, '!') {
            continue;
        }
        let Some(TokenTree::Ident(name)) = tokens.get(index + 2) else {
            continue;
        };
        if has_macro_export(&tokens[..index]) {
            *exported.entry(name.to_string()).or_default() += 1;
        }
    }
}

/// Returns whether the attributes at the end of `tokens` include `#[macro_export]`
fn has_macro_export(tokens: &[TokenTree]) -> bool {
    let mut end = tokens.len();
    while end >= 2 {
        let (TokenTree::Punct(hash), TokenTree::Group(group)) =
            (&tokens[end - 2], &tokens[end - 1])
        else {
            return false;
        };
        if hash.as_char() != '#' || group.delimiter() != Delimiter::Bracket {
            return false;
        }
        let first = group.stream().into_iter().next();
        if let Some(TokenTree::Ident(ident)) = first
            && ident == "macro_export"
        {
            return true;
        }
        end -= 2;
    }
    false
}

/// Returns whether `tokens[index]` and the token after it form `::`
fn is_path_separator(tokens: &[TokenTree], index: usize) -> bool {
    let Some(TokenTree::Punct(first)) = tokens.get(index) else {
        return false;
    };
    first.as_char() == ':' && first.spacing() == Spacing::Joint && is_punct(tokens, index + 1, ':')
}

/// Returns whether the token before `tokens[index]` is `::` or `.`
///
/// A name after either is a segment of a longer path or a field, never the
/// name of a crate.
fn follows_path_or_field(tokens: &[TokenTree], index: usize) -> bool {
    let Some(previous) = index.checked_sub(1) else {
        return false;
    };
    let after_separator = previous
        .checked_sub(1)
        .is_some_and(|start| is_path_separator(tokens, start));
    after_separator || is_punct(tokens, previous, '.')
}

fn is_punct(tokens: &[TokenTree], index: usize, character: char) -> bool {
    let Some(TokenTree::Punct(punct)) = tokens.get(index) else {
        return false;
    };
    punct.as_char() == character
}

/// Returns the string of `name!("...")` at `tokens[index]`, with the end of the call
///
/// A call with any other argument returns `None`, because the rewrite only
/// needs calls with one plain string. A string with an escape is an error:
/// the tool would have to unescape it to know which file it names.
fn macro_literal(tokens: &[TokenTree], index: usize) -> Result<Option<(String, usize)>> {
    if !is_punct(tokens, index + 1, '!') {
        return Ok(None);
    }
    let Some(TokenTree::Group(group)) = tokens.get(index + 2) else {
        return Ok(None);
    };
    let arguments: Vec<TokenTree> = group.stream().into_iter().collect();
    let [TokenTree::Literal(literal)] = arguments.as_slice() else {
        return Ok(None);
    };
    let text = literal.to_string();
    let Some(value) = text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
    else {
        return Ok(None);
    };
    if value.contains('\\') {
        bail!("the string {text} holds an escape");
    }
    Ok(Some((value.to_owned(), group.span().byte_range().end)))
}

/// Applies `edits` to `text`
fn apply(text: &str, mut edits: Vec<Edit>) -> Result<String> {
    edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
    let mut output = String::with_capacity(text.len() + edits.len() * 16);
    let mut position = 0;
    for Edit { range, replacement } in edits {
        ensure!(
            range.start >= position,
            "two edits overlap at byte {}",
            range.start
        );
        output.push_str(&text[position..range.start]);
        output.push_str(&replacement);
        position = range.end;
    }
    output.push_str(&text[position..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewriter(sources: &[&str]) -> Rewriter {
        let crates = ["tsrs_core", "tsrs_ast"]
            .into_iter()
            .map(CrateName::new)
            .collect();
        let mut rewriter = Rewriter::new(crates);
        for source in sources {
            rewriter
                .collect_exported_macros(source)
                .expect("the source should lex");
        }
        rewriter
    }

    fn rewrite(rewriter: &Rewriter, text: &str) -> String {
        rewriter
            .rewrite(text, &CrateName::new("tsrs_ast"))
            .expect("the rewrite should succeed")
            .text
    }

    const MACROS: &str = "#[macro_export]\n#[doc(hidden)]\nmacro_rules! free { () => {} }\nmacro_rules! local { () => {} }";

    #[test]
    fn check_exported_macros_with_a_duplicate_fails() {
        let rewriter = rewriter(&[MACROS, MACROS]);

        let error = rewriter.check_exported_macros().expect_err("should fail");

        assert!(
            error
                .to_string()
                .contains("2 tsrs crates export a macro named free")
        );
    }

    #[test]
    fn rewrite_keeps_comments_and_strings() {
        let rewriter = rewriter(&[]);

        let text = rewrite(
            &rewriter,
            "// tsrs_core::x\nlet s = \"crate::y\"; /* tsrs_core */",
        );

        assert_eq!(
            text,
            "// tsrs_core::x\nlet s = \"crate::y\"; /* tsrs_core */"
        );
    }

    #[test]
    fn rewrite_leaves_a_later_path_segment_alone() {
        let rewriter = rewriter(&[]);

        let text = rewrite(&rewriter, "use other::tsrs_core::x; let y = a.tsrs_core;");

        assert_eq!(text, "use other::tsrs_core::x; let y = a.tsrs_core;");
    }

    #[test]
    fn rewrite_leaves_an_import_of_an_exported_macro_at_the_root() {
        let rewriter = rewriter(&[MACROS]);

        let text = rewrite(&rewriter, "pub use crate::free;\nuse tsrs_core::free;");

        assert_eq!(text, "pub use crate::free;\nuse crate::free;");
    }

    #[test]
    fn rewrite_leaves_crate_visibility_alone() {
        let rewriter = rewriter(&[]);

        let text = rewrite(&rewriter, "pub(crate) fn f() {}");

        assert_eq!(text, "pub(crate) fn f() {}");
    }

    #[test]
    fn rewrite_moves_a_path_to_an_exported_macro_to_the_root() {
        let rewriter = rewriter(&[MACROS]);

        let text = rewrite(&rewriter, "tsrs_core::free!(x); crate::free!(y);");

        assert_eq!(text, "crate::free!(x); crate::free!(y);");
    }

    #[test]
    fn rewrite_points_the_manifest_dir_at_the_crate() {
        let rewriter = rewriter(&[]);

        let text = rewrite(
            &rewriter,
            "concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/libs\")",
        );

        assert_eq!(
            text,
            "concat!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/tsrs_ast\"), \"/libs\")"
        );
    }

    #[test]
    fn rewrite_prefixes_a_crate_in_a_type_annotation() {
        let rewriter = rewriter(&[]);

        let text = rewrite(&rewriter, "fn f(node: tsrs_core::Node) {}");

        assert_eq!(text, "fn f(node: crate::tsrs_core::Node) {}");
    }

    #[test]
    fn rewrite_prefixes_a_crate_path_with_the_module() {
        let rewriter = rewriter(&[]);

        let text = rewrite(
            &rewriter,
            "use crate::node::Node;\npub(in crate::a) fn f() {}",
        );

        assert_eq!(
            text,
            "use crate::tsrs_ast::node::Node;\npub(in crate::tsrs_ast::a) fn f() {}"
        );
    }

    #[test]
    fn rewrite_prefixes_a_dollar_crate_path_with_the_module() {
        let rewriter = rewriter(&[MACROS]);

        let text = rewrite(
            &rewriter,
            "macro_rules! m { () => { $crate::ptr::f(); $crate::free!() } }",
        );

        assert_eq!(
            text,
            "macro_rules! m { () => { $crate::tsrs_ast::ptr::f(); $crate::free!() } }"
        );
    }

    #[test]
    fn rewrite_prefixes_a_module_that_shares_a_name_with_a_macro() {
        let rewriter = rewriter(&[MACROS]);

        let text = rewrite(&rewriter, "crate::free::release(x);");

        assert_eq!(text, "crate::tsrs_ast::free::release(x);");
    }

    #[test]
    fn rewrite_prefixes_a_name_of_a_crate_with_crate() {
        let rewriter = rewriter(&[]);

        let text = rewrite(&rewriter, "use tsrs_core as core;\nuse tsrs_core::{a, b};");

        assert_eq!(
            text,
            "use crate::tsrs_core as core;\nuse crate::tsrs_core::{a, b};"
        );
    }

    #[test]
    fn rewrite_prefixes_paths_inside_macro_bodies() {
        let rewriter = rewriter(&[]);

        let text = rewrite(&rewriter, "vec![tsrs_core::A, crate::B]");

        assert_eq!(text, "vec![crate::tsrs_core::A, crate::tsrs_ast::B]");
    }

    #[test]
    fn rewrite_records_included_files() {
        let rewriter = rewriter(&[]);

        let rewritten = rewriter
            .rewrite(
                "const A: &str = include_str!(\"../libs/a.d.ts\");",
                &CrateName::new("tsrs_ast"),
            )
            .expect("the rewrite should succeed");

        assert_eq!(rewritten.includes(), ["../libs/a.d.ts".to_owned()]);
    }

    #[test]
    fn rewrite_with_an_escaped_include_fails() {
        let rewriter = rewriter(&[]);

        let error = rewriter
            .rewrite("include_str!(\"a\\\\b\")", &CrateName::new("tsrs_ast"))
            .expect_err("should fail");

        assert!(format!("{error:#}").contains("holds an escape"));
    }

    #[test]
    fn trait_auto_crate_name() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<CrateName>();
    }

    #[test]
    fn trait_auto_edit() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Edit>();
    }

    #[test]
    fn trait_auto_rewriter() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Rewriter>();
    }

    #[test]
    fn trait_auto_rewritten() {
        fn assert_auto<T: Send + Sync + Unpin>() {}
        assert_auto::<Rewritten>();
    }
}
