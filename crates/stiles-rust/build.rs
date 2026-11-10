/// Generates the lint pass trait and fingerprints it for plugins
///
/// The script also tells the crate whether cargo packaged it, which
/// `PROVENANCE` reads.
///
/// The fingerprint covers the generated code alone, because the rest of this
/// crate's source does not shape the plugin boundary. Hashing it would refuse
/// every plugin in the tree over an edit that moved nothing.
fn main() {
    stiles_codegen::build_provenance(&mut std::io::stdout())
        .expect("failed to tell the crate whether cargo packaged it");
    stiles_codegen::build_language("Rust", &["src/node-types.json"], &mut std::io::stdout())
        .expect("failed to generate the Rust lint pass");
}
