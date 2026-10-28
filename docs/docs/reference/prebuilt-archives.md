# Prebuilt archives

<!--
goal: let a consumer read what happened when stiles did or did not use
published archives.
non-goal: how to publish them. that is the reference page of the same name in
the authoring tree.
-->

When using [shared lint rules][shared-rules], Stiles can use prebuilt archives
published to Github Releases. This avoids recompiling the same lints every time
you run Stiles without a cache.

## Lookup order

Stiles caches shared rules locally on your machine, by commit hash. When a
local cache is available, it will not look for prebuilt archives on Github. This
might lead to unexpected recompilation of rules.

To resolve this, deleting the [Stiles cache][cache-layout] in
`<cache>/prebuilt/<remote>/<rev>/` will make Stiles look for archives.

[shared-rules]: /docs/how-to/shared-rules
[cache-layout]: /docs/reference/cache-layout
