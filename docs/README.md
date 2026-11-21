# Documentation

The public website and documentation for Stiles is built using [Docusaurus].

[Mise] provides Node.js at the version that `mise.toml` in the project root
pins. With mise active in your shell, start the development server from this
directory:

```sh
npm ci
npm run start
```

A Cloudflare Worker serves the site from `build`, as `wrangler.jsonc`
configures it. Every published release deploys the site to `stiles.aonyx.ai`,
and so does a manual run of the workflow Docs on `main`. Every pull request
from this repository uploads a preview to
`https://pr-<number>-stiles.aonyx.workers.dev`, and the pull request links to
it. Only the team can open a preview.

[docusaurus]: https://docusaurus.io/
[mise]: https://mise.jdx.dev/
