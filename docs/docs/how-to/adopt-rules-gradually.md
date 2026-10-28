# Adopt rules gradually

<!--
goal: let a project that fails many rules start using stiles today.
non-goal: deciding which rules are worth turning on. that is each rule's own
documentation.
-->

Adopting Stiles rules can be an arduous process, requiring significant changes
to your codebase. To avoid this, we have multiple ways to adopt only specific
rules. These are explained on this page.

## Enable only specific rules

You can choose to run only particular rules. This disables all other rules. To
do so, add a `[rules]` section to your configuration with an `enable` property
like so:

```toml title=".config/stiles.toml"
[rules]
enable = ["lint.doc-summary-break"]
```

## Disable rules

When a new rule comes in, or a particular rule doesn't fit your style, you can
choose to disable only that rule. To do so, add a `disable` property to your
configuration:

```toml title=".config/stiles.toml"
[rules]
disable = ["lint.no-inline-comments"]
```

## Find a rule's name

To enable and disable rules, you need to know the rule's ID. Run this in your
project to list the rules your configured lints declare:

```bash
stiles rules
```

Each line names a rule and the languages it runs on, and ends in `off` when
your `[rules]` turn it off.

## Fine-grained control

Rules may define more fine-grained controls than just enable/disable. These are
called rule options, and configured in the [config file][rule-options].

[rule-options]: /docs/reference/configuration#rule-options
