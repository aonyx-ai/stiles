/// Tells the crate whether cargo packaged it
///
/// [`PROVENANCE`] reads the flag, and stiles-plugin joins it with the
/// flags of the other boundary crates.
///
/// [`PROVENANCE`]: crate::plugin::PROVENANCE
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    stiles_codegen::build_provenance(&mut std::io::stdout())
        .expect("failed to tell the crate whether cargo packaged it");
}
