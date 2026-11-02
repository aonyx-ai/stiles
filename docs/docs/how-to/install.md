# Install Stiles

There's various ways to install Stiles. On this page we'll go through a few
options.

## Recommended: Install script

This is easily the fastest way to get started. Feel free to read through [the
installation script][install-sh] to make sure it's safe to run.

```bash
curl -LsSf https://opensource.aonyx.ai/stiles/install.sh | sh
```

If you don't want to install in `$HOME/.local/bin` you can set a different path
in the `STILES_INSTALL_DIR` environment variable and the script will pick that
up.

## Github Releases

Alternatively, grab the `.tar.gz` and the shasum directly from [Github
Releases][gh-releases]. You'll have to move the file someplace on your `$PATH`
yourself.

```bash
shasum -a 256 -c stiles-0.1.0-aarch64-apple-darwin.tar.gz.sha256
tar -xzf stiles-0.1.0-aarch64-apple-darwin.tar.gz
```

## With Cargo

Stiles is on crates.io. [cargo-binstall][binstall] downloads the archive of a
release and installs it without a build:

```bash
cargo binstall stiles
```

`cargo install` builds the same release from source:

```bash
cargo install --locked stiles
```

## From source

To install an unreleased version, point `cargo install` at the Git
repository:

```bash
cargo install --git https://github.com/aonyx-ai/stiles --locked stiles
```

Alternatively, you can clone the repository and install it from there:

```bash
git clone https://github.com/aonyx-ai/stiles
cd stiles
cargo install --locked --path crates/stiles
```

[binstall]: https://github.com/cargo-bins/cargo-binstall
[install-sh]: https://github.com/aonyx-ai/stiles/blob/main/docs/static/install.sh
[gh-releases]: https://github.com/aonyx-ai/stiles/releases
