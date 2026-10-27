# Documentation

The public website and documentation for Whisker is built using [Docusaurus].

[Mise] provides Node.js at the version that `mise.toml` in the project root
pins. With mise active in your shell, start the development server from this
directory:

```sh
npm ci
npm run start
```

The site is built on every pull request. It is deployed to GitHub Pages when a
release is published.

[docusaurus]: https://docusaurus.io/
[mise]: https://mise.jdx.dev/
