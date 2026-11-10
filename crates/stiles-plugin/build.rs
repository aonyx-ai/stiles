/// Tells the crate whether cargo packaged it
///
/// [`PROVENANCE`] joins the flag with the provenance of every boundary
/// crate the plugin builds on.
///
/// [`PROVENANCE`]: crate::PROVENANCE
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    stiles_codegen::build_provenance(&mut std::io::stdout())
        .expect("failed to tell the crate whether cargo packaged it");
}
