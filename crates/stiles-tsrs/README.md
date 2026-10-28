# stiles-tsrs

The TypeScript checker of [tsrs], repackaged as one crate.

tsrs publishes no crates. This crate holds the tsrs crates that
stiles uses, each as a module of the same name, from commit
`4a61321929d0d62b86c5fb35f15bfa1607e03b50`:

- `tsrs_ast`
- `tsrs_astnav`
- `tsrs_binder`
- `tsrs_checker`
- `tsrs_compiler`
- `tsrs_contentmapper`
- `tsrs_core`
- `tsrs_declarations`
- `tsrs_diagnostics`
- `tsrs_ipc`
- `tsrs_module`
- `tsrs_modulespecifiers`
- `tsrs_parser`
- `tsrs_printer`
- `tsrs_pseudochecker`
- `tsrs_scanner`
- `tsrs_sourcemap`
- `tsrs_spanmap`
- `tsrs_transformers`
- `tsrs_tsoptions`
- `tsrs_vfs`

A tool generates the crate, and the only change to the code is to the
paths that name a crate. The code is the work of the tsrs authors,
under the license and the notice beside this file.

[tsrs]: https://github.com/oxc-project/tsrs
