/// Generates the lint pass trait and fingerprints it for plugins
///
/// TypeScript parses with two grammars: `.tsx` files need one that knows
/// JSX, which in turn drops the angle-bracket type assertion. The trait is
/// generated from the union of both grammars' node kinds, so one rule runs
/// on either kind of file.
fn main() {
    whisker_codegen::build_language(
        "TypeScript",
        &["src/typescript-node-types.json", "src/tsx-node-types.json"],
    )
    .expect("failed to generate the TypeScript lint pass");
}
