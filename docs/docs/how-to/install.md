# Install Stiles

There's various ways to install Stiles. On this page we'll go through a few
options.

## Recommended: Install script

This is easily the fastest way to get started. Feel free to read through [the
installation script][install-sh] to make sure it's safe to run.

```bash
curl -LsSf https://aonyx-ai.github.io/stiles/install.sh | sh
```

If you don't want to install in `$HOME/.local/bin` you can set a different path
in the `STILES_INSTALL_DIR` environment variable and the script will pick that
up.

## Github Releases

Alternatively, grab the `.tar.gz` and the shasum directly from [Github
Releases][gh-releases]. You'll have to move the file someplace on your `$PATH`
yourself.

```bash
shasum -a 256 -c stiles-0.1.0-rc.5-aarch64-apple-darwin.tar.gz.sha256
tar -xzf stiles-0.1.0-rc.5-aarch64-apple-darwin.tar.gz
```

## From source

If your platform has no archive, you want to install an unreleased version, or
you don't want to install binary files from the internet, you can easily build
Stiles from source. The easiest method is to use `cargo install` pointed at the
Git repository:

```bash
cargo install --git https://github.com/aonyx-ai/stiles --locked stiles
```

Alternatively, you can clone the repository and install it from there:

```bash
git clone https://github.com/aonyx-ai/stiles
cd stiles
cargo install --locked --path crates/stiles
```

[install-sh]: https://github.com/aonyx-ai/stiles/blob/main/docs/static/install.sh
[gh-releases]: https://github.com/aonyx-ai/stiles/releases
