# Temporary Oxigraph parser security patches

`oxrdfxml-0.2.3` and `sparesults-0.3.3` carry the upstream quick-xml 0.40 API
adaptations from Oxigraph commit
`822b7c9462dea8b525fed3cb8150bc3e9b1c243b`, applied to the published crate
versions. Their normalized manifests retain the registry dependency graph except
for this security update:

```toml
quick-xml = "0.41"
```

Oxigraph 0.5.x published these parser crates with `quick-xml = "0.37"`, which
is affected by RUSTSEC-2026-0194 and RUSTSEC-2026-0195. Upstream confirmed that
the same parser code supports quick-xml 0.41 in commit
`e115a6a8dd9213fdf89a20cb72494ab333878218` on 2026-07-10, but has not yet
published compatible parser crate versions. These local path patches avoid
mixing registry Oxigraph types with git-workspace path dependencies while
preserving the Oxigraph 0.5.8 package boundary.

The source remains licensed under MIT OR Apache-2.0; each snapshot carries the
upstream license texts. Remove the `[patch.crates-io]` entries and these
snapshots after Oxigraph publishes compatible parser crate versions that resolve
to `quick-xml >=0.41.0`.

References:

- <https://github.com/oxigraph/oxigraph/commit/822b7c9462dea8b525fed3cb8150bc3e9b1c243b>
- <https://github.com/oxigraph/oxigraph/commit/e115a6a8dd9213fdf89a20cb72494ab333878218>
- <https://rustsec.org/advisories/RUSTSEC-2026-0194.html>
- <https://rustsec.org/advisories/RUSTSEC-2026-0195.html>
