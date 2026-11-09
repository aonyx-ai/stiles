use std::collections::BTreeSet;

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use stiles_types::Language;

use super::DocumentedRule;
use super::sections::{Pair, Sections};

/// The style of the page, inlined so that the page is one file
const STYLE: &str = include_str!("page.css");

/// The search and the filter of the page
const SCRIPT: &str = include_str!("page.js");

/// Returns the reference page of `rules` as one HTML document
pub(super) fn render(title: &str, rules: &[DocumentedRule]) -> String {
    let title = escape(title);
    let entries: String = rules.iter().map(render_rule).collect();
    let filter = render_filter(rules);

    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title}</title>\n<style>\n{STYLE}</style>\n</head>\n<body>\n\
         <div class=\"controls\">\n\
         <input type=\"search\" placeholder=\"Search rules\" aria-label=\"Search rules\">\n\
         {filter}<output></output>\n</div>\n<main>\n{entries}</main>\n\
         <p class=\"empty\" hidden>No rules match</p>\n<script>\n{SCRIPT}</script>\n\
         </body>\n</html>\n"
    )
}

/// Returns the language buttons, or nothing when only one language exists
fn render_filter(rules: &[DocumentedRule]) -> String {
    let languages: BTreeSet<&str> = rules
        .iter()
        .flat_map(|rule| rule.languages.iter().copied().map(language_name))
        .collect();
    if languages.len() < 2 {
        return String::new();
    }

    let buttons: String = languages
        .into_iter()
        .map(|language| {
            format!(
                "<button type=\"button\" data-language=\"{language}\" \
                 aria-pressed=\"false\">{language}</button>\n"
            )
        })
        .collect();

    format!(
        "<button type=\"button\" data-language=\"\" aria-pressed=\"true\">All</button>\n{buttons}"
    )
}

/// Returns one collapsed entry of the page
fn render_rule(rule: &DocumentedRule) -> String {
    let DocumentedRule {
        id,
        languages,
        docs,
    } = rule;
    let Sections {
        summary,
        body,
        pair,
    } = Sections::parse(docs);

    let id = escape(id);
    let names: Vec<&str> = languages.iter().copied().map(language_name).collect();
    let attribute = names.join(" ");
    let shown = names.join(", ");
    let search = escape(&format!("{id} {docs}").to_lowercase());
    let summary = render_inline(&summary);
    let body = render_markdown(&body);
    let pair = pair.as_ref().map(render_pair).unwrap_or_default();

    format!(
        "<details id=\"{id}\" data-languages=\"{attribute}\" data-search=\"{search}\">\n\
         <summary><code class=\"id\">{id}</code><span class=\"text\">{summary}</span>\
         <span class=\"languages\">{shown}</span></summary>\n\
         <div class=\"docs\">{body}</div>\n{pair}</details>\n"
    )
}

/// Returns the negative example and its fix, side by side
fn render_pair(pair: &Pair) -> String {
    let Pair { example, fix } = pair;
    let example = escape(example);
    let fix = escape(fix);

    format!(
        "<div class=\"pair\">\n\
         <figure class=\"dont\"><figcaption>Don't</figcaption>\
         <pre><code>{example}</code></pre></figure>\n\
         <figure class=\"do\"><figcaption>Do</figcaption>\
         <pre><code>{fix}</code></pre></figure>\n</div>\n"
    )
}

/// Returns the name a reader of the page sees for a language
fn language_name(language: Language) -> &'static str {
    match language {
        Language::Rust => "Rust",
        Language::TypeScript => "TypeScript",
    }
}

/// Returns a Markdown paragraph as inline HTML
fn render_inline(markdown: &str) -> String {
    let html = render_markdown(markdown);
    let html = html.trim_end();
    let html = html.strip_prefix("<p>").unwrap_or(html);

    html.strip_suffix("</p>").unwrap_or(html).to_owned()
}

/// Returns Markdown from a rule's docs as HTML
///
/// A rustdoc link names an item in code, and the rule crates are published
/// nowhere it could point. Only its text stays, and a web link stays whole.
/// Raw HTML turns into text, so a stray `<T>` in the docs shows as written
/// and the docs cannot script the page. Headings move down two levels to
/// sit under an entry.
fn render_markdown(markdown: &str) -> String {
    let mut web_links = Vec::new();
    let events = Parser::new_with_broken_link_callback(
        markdown,
        Options::empty(),
        Some(|_| Some((CowStr::Borrowed(""), CowStr::Borrowed("")))),
    )
    .filter_map(|event| match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let web = is_web_url(&dest_url);
            web_links.push(web);
            web.then_some(Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }))
        }
        Event::End(TagEnd::Link) => web_links
            .pop()
            .unwrap_or_default()
            .then_some(Event::End(TagEnd::Link)),
        Event::Start(Tag::Heading {
            level,
            id,
            classes,
            attrs,
        }) => Some(Event::Start(Tag::Heading {
            level: lowered(level),
            id,
            classes,
            attrs,
        })),
        Event::End(TagEnd::Heading(level)) => Some(Event::End(TagEnd::Heading(lowered(level)))),
        Event::Html(text) | Event::InlineHtml(text) => Some(Event::Text(text)),
        other => Some(other),
    });

    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    html
}

/// Returns whether a link target is a page on the web
fn is_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Returns the heading level two below `level`
fn lowered(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H3,
        HeadingLevel::H2 => HeadingLevel::H4,
        HeadingLevel::H3 => HeadingLevel::H5,
        HeadingLevel::H4 | HeadingLevel::H5 | HeadingLevel::H6 => HeadingLevel::H6,
    }
}

/// Returns text that is safe inside HTML content and attributes
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(languages: &[Language]) -> DocumentedRule {
        DocumentedRule {
            id: "lint.todo".to_owned(),
            languages: languages.to_vec(),
            docs: "Flags a `thing`\n\nA thing hides intent.\n\n# Examples\n\n```text\n// DON'T\na<b\n\n// DO\nc\n```\n".to_owned(),
        }
    }

    #[test]
    fn escape_covers_attribute_quotes() {
        let escaped = escape("\"<&'>");

        assert_eq!(escaped, "&quot;&lt;&amp;&#39;&gt;");
    }

    #[test]
    fn render_escapes_the_title() {
        let page = render("<rules>", &[]);

        assert!(page.contains("<title>&lt;rules&gt;</title>"));
    }

    #[test]
    fn render_filter_with_one_language_is_empty() {
        let filter = render_filter(&[rule(&[Language::Rust])]);

        assert_eq!(filter, "");
    }

    #[test]
    fn render_filter_with_two_languages_has_a_button_for_each() {
        let filter = render_filter(&[rule(&[Language::Rust, Language::TypeScript])]);

        assert_eq!(filter.matches("<button").count(), 3);
    }

    #[test]
    fn render_inline_drops_the_paragraph() {
        let html = render_inline("Flags a `thing`");

        assert_eq!(html, "Flags a <code>thing</code>");
    }

    #[test]
    fn render_markdown_keeps_a_web_link() {
        let html = render_markdown("See [TOML].\n\n[TOML]: https://toml.io");

        assert!(html.contains("<a href=\"https://toml.io\">TOML</a>"));
    }

    #[test]
    fn render_markdown_lowers_a_heading() {
        let html = render_markdown("# Options");

        assert_eq!(html.trim(), "<h3>Options</h3>");
    }

    #[test]
    fn render_markdown_reduces_a_defined_rustdoc_link_to_its_text() {
        let html = render_markdown("See [`Item`].\n\n[`Item`]: std::option::Option");

        assert_eq!(html.trim(), "<p>See <code>Item</code>.</p>");
    }

    #[test]
    fn render_markdown_reduces_a_rustdoc_link_to_its_code() {
        let html = render_markdown("See [`boundary::DERIVES`].");

        assert_eq!(html.trim(), "<p>See <code>boundary::DERIVES</code>.</p>");
    }

    #[test]
    fn render_markdown_shows_raw_html_as_text() {
        let html = render_markdown("A <script>alert(1)</script> here");

        assert!(!html.contains("<script>"));
    }

    #[test]
    fn render_rule_escapes_the_example() {
        let entry = render_rule(&rule(&[Language::Rust]));

        assert!(entry.contains("<code>a&lt;b</code>"));
    }

    #[test]
    fn render_rule_makes_the_id_an_anchor() {
        let entry = render_rule(&rule(&[Language::Rust]));

        assert!(entry.starts_with("<details id=\"lint.todo\""));
    }

    #[test]
    fn render_rule_names_both_languages() {
        let entry = render_rule(&rule(&[Language::Rust, Language::TypeScript]));

        assert!(entry.contains("Rust, TypeScript"));
    }
}
